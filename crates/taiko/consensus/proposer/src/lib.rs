//! A [Consensus] implementation for local testing purposes
//! that automatically seals blocks.
//!
//! The Mining task polls a [`MiningMode`], and will return a list of transactions that are ready to
//! be mined.
//!
//! These downloaders poll the miner, assemble the block, and return transactions that are ready to
//! be mined.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]

use reth_chainspec::ChainSpec;
use reth_consensus::{Consensus, ConsensusError, PostExecutionInput};
use reth_errors::RethError;
use reth_execution_errors::{BlockExecutionError, BlockValidationError};
use reth_primitives::{
    eip4844::calculate_excess_blob_gas, proofs, Address, Block,
    BlockWithSenders, Header, Requests, SealedBlock, SealedHeader, TransactionSigned, Withdrawals,
    U256,
};
use reth_provider::{BlockReaderIdExt, StateProviderFactory};
use reth_revm::database::StateProviderDatabase;
use reth_transaction_pool::TransactionPool;
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::oneshot;
use tracing::debug;

mod client;
mod task;

pub use crate::client::ProposerClient;
use reth_evm::execute::{BlockExecutionInput, BlockExecutionOutput, BlockExecutorProvider, Executor, TaskResult};
pub use task::ProposerTask;

/// A consensus implementation intended for local development and testing purposes.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProposerConsensus {
    /// Configuration
    chain_spec: Arc<ChainSpec>,
}

impl ProposerConsensus {
    /// Create a new instance of [`MinerConsensus`]
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self { chain_spec }
    }
}

impl Consensus for ProposerConsensus {
    fn validate_header(&self, _header: &SealedHeader) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        _header: &SealedHeader,
        _parent: &SealedHeader,
    ) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn validate_header_with_total_difficulty(
        &self,
        _header: &Header,
        _total_difficulty: U256,
    ) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn validate_block_pre_execution(&self, _block: &SealedBlock) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn validate_block_post_execution(
        &self,
        _block: &BlockWithSenders,
        _input: PostExecutionInput<'_>,
    ) -> Result<(), ConsensusError> {
        Ok(())
    }
}

/// Builder type for configuring the setup
#[derive(Debug)]
pub struct ProposerBuilder<Provider, Pool, BlockExecutor> {
    provider: Provider,
    consensus: ProposerConsensus,
    pool: Pool,
    block_executor: BlockExecutor,
}

impl<Provider, Pool, BlockExecutor> ProposerBuilder<Provider, Pool, BlockExecutor>
where
    Pool: TransactionPool,
{
    /// Creates a new builder instance to configure all parts.
    pub fn new(
        chain_spec: Arc<ChainSpec>,
        provider: Provider,
        pool: Pool,
        block_executor: BlockExecutor,
    ) -> Self {
        Self { provider, consensus: ProposerConsensus::new(chain_spec), pool, block_executor }
    }

    /// Consumes the type and returns all components
    #[track_caller]
    pub fn build(
        self,
    ) -> (ProposerConsensus, ProposerClient, ProposerTask<Provider, Pool, BlockExecutor>) {
        let Self { provider: client, consensus, pool, block_executor: evm_config } = self;
        let (trigger_args_tx, trigger_args_rx) = tokio::sync::mpsc::unbounded_channel();
        let auto_client = ProposerClient::new(trigger_args_tx);
        let task = ProposerTask::new(
            Arc::clone(&consensus.chain_spec),
            client,
            pool,
            evm_config,
            trigger_args_rx,
        );
        (consensus, auto_client, task)
    }
}

/// Arguments for the trigger
#[derive(Debug)]
pub struct TaskArgs {
    /// Address of the beneficiary
    pub beneficiary: Address,
    /// Base fee
    pub base_fee: u64,
    /// Maximum gas limit for the block
    pub block_max_gas_limit: u64,
    /// Maximum bytes per transaction list
    pub max_bytes_per_tx_list: u64,
    /// Local accounts
    pub local_accounts: Option<Vec<Address>>,
    /// Maximum number of transactions lists
    pub max_transactions_lists: u64,
    /// Minimum tip
    pub min_tip: u64,

    tx: oneshot::Sender<Result<Vec<TaskResult>, RethError>>,
}

#[derive(Debug, Clone, Default)]
struct Storage;

impl Storage {
    /// Fills in pre-execution header fields based on the current best block and given
    /// transactions.
    #[allow(clippy::too_many_arguments)]
    fn build_header_template<Provider>(
        timestamp: u64,
        transactions: &[TransactionSigned],
        ommers: &[Header],
        provider: &Provider,
        withdrawals: Option<&Withdrawals>,
        requests: Option<&Requests>,
        chain_spec: &ChainSpec,
        beneficiary: Address,
        block_max_gas_limit: u64,
        base_fee: u64,
    ) -> Result<Header, BlockExecutionError>
    where
        Provider: BlockReaderIdExt,
    {
        let base_fee_per_gas = Some(base_fee);

        let blob_gas_used = if chain_spec.is_cancun_active_at_timestamp(timestamp) {
            let mut sum_blob_gas_used = 0;
            for tx in transactions {
                if let Some(blob_tx) = tx.transaction.as_eip4844() {
                    sum_blob_gas_used += blob_tx.blob_gas();
                }
            }
            Some(sum_blob_gas_used)
        } else {
            None
        };
        let latest_block =
            provider.latest_header().map_err(BlockExecutionError::LatestBlock)?.unwrap();
        let mut header = Header {
            parent_hash: latest_block.hash(),
            ommers_hash: proofs::calculate_ommers_root(ommers),
            beneficiary,
            state_root: Default::default(),
            transactions_root: proofs::calculate_transaction_root(transactions),
            receipts_root: Default::default(),
            withdrawals_root: withdrawals.map(|w| proofs::calculate_withdrawals_root(w)),
            logs_bloom: Default::default(),
            difficulty: U256::ZERO,
            number: latest_block.number + 1,
            gas_limit: block_max_gas_limit,
            gas_used: 0,
            timestamp,
            mix_hash: Default::default(),
            nonce: 0,
            base_fee_per_gas,
            blob_gas_used,
            excess_blob_gas: None,
            extra_data: Default::default(),
            parent_beacon_block_root: None,
            requests_root: requests.map(|r| proofs::calculate_requests_root(&r.0)),
        };

        if chain_spec.is_cancun_active_at_timestamp(timestamp) {
            header.parent_beacon_block_root = latest_block.parent_beacon_block_root;
            header.blob_gas_used = Some(0);

            let (parent_excess_blob_gas, parent_blob_gas_used) =
                if chain_spec.is_cancun_active_at_timestamp(latest_block.timestamp) {
                    (
                        latest_block.excess_blob_gas.unwrap_or_default(),
                        latest_block.blob_gas_used.unwrap_or_default(),
                    )
                } else {
                    (0, 0)
                };

            header.excess_blob_gas =
                Some(calculate_excess_blob_gas(parent_excess_blob_gas, parent_blob_gas_used))
        }

        Ok(header)
    }

    /// Builds and executes a new block with the given transactions, on the provided executor.
    ///
    /// This returns the header of the executed block, as well as the poststate from execution.
    #[allow(clippy::too_many_arguments)]
    fn build_and_execute<Provider, Executor>(
        transactions: Vec<TransactionSigned>,
        ommers: Vec<Header>,
        provider: &Provider,
        chain_spec: Arc<ChainSpec>,
        executor: &Executor,
        beneficiary: Address,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        max_transactions_lists: u64,
        base_fee: u64,
    ) -> Result<Vec<TaskResult>, RethError>
    where
        Executor: BlockExecutorProvider,
        Provider: StateProviderFactory + BlockReaderIdExt,
    {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

        // if shanghai is active, include empty withdrawals
        let withdrawals =
            chain_spec.is_shanghai_active_at_timestamp(timestamp).then_some(Withdrawals::default());
        // if prague is active, include empty requests
        let requests =
            chain_spec.is_prague_active_at_timestamp(timestamp).then_some(Requests::default());

        let header = Self::build_header_template(
            timestamp,
            &transactions,
            &ommers,
            provider,
            withdrawals.as_ref(),
            requests.as_ref(),
            &chain_spec,
            beneficiary,
            block_max_gas_limit,
            base_fee,
        )?;

        let mut block = Block { header, body: transactions, ommers, withdrawals, requests }
            .with_recovered_senders()
            .ok_or(BlockExecutionError::Validation(BlockValidationError::SenderRecoveryError))?;

        debug!(target: "taiko::proposer", transactions=?&block.body, "before executing transactions");

        let mut db = StateProviderDatabase::new(
            provider.latest().map_err(BlockExecutionError::LatestBlock)?,
        );

        // execute the block
        let block_input = BlockExecutionInput {
            block: &mut block,
            total_difficulty: U256::ZERO,
            enable_anchor: false,
            enable_skip: false,
            enable_build: true,
            max_bytes_per_tx_list,
            max_transactions_lists,
        };
        let BlockExecutionOutput { target_list, .. } =
            executor.executor(&mut db).execute(block_input.into())?;

        Ok(target_list)
    }
}
