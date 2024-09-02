//! Taiko's payload builder module.

use std::sync::Arc;

use crate::error::TaikoPayloadBuilderError;
use reth_basic_payload_builder::*;
use reth_chainspec::ChainSpec;
use reth_errors::{BlockExecutionError, BlockValidationError};
use reth_evm::{
    execute::{BlockExecutionOutput, BlockExecutorProvider, Executor},
    ConfigureEvm,
};
use reth_payload_builder::error::PayloadBuilderError;
use reth_primitives::{
    constants::BEACON_NONCE, eip4844::calculate_excess_blob_gas, proofs, Block, Bloom, Header,
    TransactionSigned, EMPTY_OMMER_ROOT_HASH, U256,
};
use reth_provider::{ExecutionOutcome, StateProviderFactory};
use reth_revm::database::StateProviderDatabase;
use reth_transaction_pool::TransactionPool;
use taiko_reth_engine_primitives::{TaikoBuiltPayload, TaikoPayloadBuilderAttributes};
use taiko_reth_evm::{execute::TaikoExecutorProvider, TaikoEvmConfig};
use taiko_reth_primitives::L1Origin;
use taiko_reth_provider::L1OriginWriter;
use tracing::debug;

/// Taiko's payload builder
#[derive(Debug, Clone)]
pub struct TaikoPayloadBuilder<EvmConfig = TaikoEvmConfig> {
    /// The type responsible for creating the evm.
    block_executor: TaikoExecutorProvider<EvmConfig>,
}

impl<EvmConfig: Clone> TaikoPayloadBuilder<EvmConfig> {
    /// `OptimismPayloadBuilder` constructor.
    pub fn new(evm_config: EvmConfig, chain_spec: Arc<ChainSpec>) -> Self {
        let _evm_config = evm_config.clone();
        let block_executor = TaikoExecutorProvider::new(chain_spec, evm_config);
        Self { block_executor }
    }
}

/// Implementation of the [`PayloadBuilder`] trait for [`TaikoPayloadBuilder`].
impl<Pool, Client, EvmConfig> PayloadBuilder<Pool, Client> for TaikoPayloadBuilder<EvmConfig>
where
    Client: StateProviderFactory + L1OriginWriter,
    Pool: TransactionPool,
    EvmConfig: ConfigureEvm,
{
    type Attributes = TaikoPayloadBuilderAttributes;
    type BuiltPayload = TaikoBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<Pool, Client, TaikoPayloadBuilderAttributes, TaikoBuiltPayload>,
    ) -> Result<BuildOutcome<TaikoBuiltPayload>, PayloadBuilderError> {
        let BuildArguments { cached_reads, .. } = args;
        Ok(BuildOutcome::Aborted { fees: U256::ZERO, cached_reads })
    }

    fn build_empty_payload(
        &self,
        client: &Client,
        config: PayloadConfig<Self::Attributes>,
    ) -> Result<TaikoBuiltPayload, PayloadBuilderError> {
        taiko_payload_builder(client, &self.block_executor, config)
    }
}

/// Constructs an Ethereum transaction payload using the best transactions from the pool.
///
/// Given build arguments including an Ethereum client, transaction pool,
/// and configuration, this function creates a transaction payload. Returns
/// a result indicating success with the payload or an error in case of failure.
#[inline]
fn taiko_payload_builder<EvmConfig, Client>(
    client: &Client,
    executor: &TaikoExecutorProvider<EvmConfig>,
    config: PayloadConfig<TaikoPayloadBuilderAttributes>,
) -> Result<TaikoBuiltPayload, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm,
    Client: StateProviderFactory + L1OriginWriter,
{
    let state_provider = client.state_by_block_hash(config.parent_block.hash())?;
    let mut db = StateProviderDatabase::new(state_provider);
    let PayloadConfig { initialized_block_env, parent_block, attributes, chain_spec, .. } = config;

    debug!(target: "taiko_payload_builder", id=%attributes.payload_attributes.payload_id(), parent_hash = ?parent_block.hash(), parent_number = parent_block.number, "building new payload");
    let block_gas_limit: u64 = initialized_block_env.gas_limit.try_into().unwrap_or(u64::MAX);
    let base_fee = initialized_block_env.basefee.to::<u64>();

    let block_number = initialized_block_env.number.to::<u64>();

    let transactions: Vec<TransactionSigned> =
        alloy_rlp::Decodable::decode(&mut attributes.block_metadata.tx_list.as_ref())
            .map_err(|_| PayloadBuilderError::other(TaikoPayloadBuilderError::FailedToDecodeTx))?;

    // initialize empty blob sidecars at first. If cancun is active then this will
    let mut excess_blob_gas = None;
    let mut blob_gas_used = None;

    // only determine cancun fields when active
    if chain_spec.is_cancun_active_at_timestamp(attributes.payload_attributes.timestamp) {
        excess_blob_gas = if chain_spec.is_cancun_active_at_timestamp(parent_block.timestamp) {
            let parent_excess_blob_gas = parent_block.excess_blob_gas.unwrap_or_default();
            let parent_blob_gas_used = parent_block.blob_gas_used.unwrap_or_default();
            Some(calculate_excess_blob_gas(parent_excess_blob_gas, parent_blob_gas_used))
        } else {
            // for the first post-fork block, both parent.blob_gas_used and
            // parent.excess_blob_gas are evaluated as 0
            Some(calculate_excess_blob_gas(0, 0))
        };

        blob_gas_used = Some(0);
    }
    // let mut header = Header {
    //     parent_hash: self.best_hash,
    //     ommers_hash: proofs::calculate_ommers_root(ommers),
    //     beneficiary: Default::default(),
    //     state_root: Default::default(),
    //     transactions_root: proofs::calculate_transaction_root(transactions),
    //     receipts_root: Default::default(),
    //     withdrawals_root: withdrawals.map(|w| proofs::calculate_withdrawals_root(w)),
    //     logs_bloom: Default::default(),
    //     difficulty: U256::from(2),
    //     number: self.best_block + 1,
    //     gas_limit: ETHEREUM_BLOCK_GAS_LIMIT,
    //     gas_used: 0,
    //     timestamp,
    //     mix_hash: Default::default(),
    //     nonce: 0,
    //     base_fee_per_gas,
    //     blob_gas_used,
    //     excess_blob_gas: None,
    //     extra_data: Default::default(),
    //     parent_beacon_block_root: None,
    //     requests_root: requests.map(|r| proofs::calculate_requests_root(&r.0)),
    // };
    let header = Header {
        parent_hash: parent_block.hash(),
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary: initialized_block_env.coinbase,
        state_root: Default::default(),
        transactions_root: Default::default(),
        receipts_root: Default::default(),
        withdrawals_root: Some(proofs::calculate_withdrawals_root(
            &attributes.payload_attributes.withdrawals,
        )),
        logs_bloom: Default::default(),
        timestamp: attributes.payload_attributes.timestamp,
        mix_hash: initialized_block_env.prevrandao.unwrap(),
        nonce: BEACON_NONCE,
        base_fee_per_gas: Some(base_fee),
        number: block_number,
        gas_limit: block_gas_limit,
        difficulty: initialized_block_env.difficulty,
        gas_used: 0,
        extra_data: attributes.block_metadata.extra_data.clone().into(),
        parent_beacon_block_root: attributes.payload_attributes.parent_beacon_block_root,
        blob_gas_used,
        excess_blob_gas,
        requests_root: Default::default(),
    };

    // seal the block
    let mut block = Block {
        header,
        body: transactions,
        ommers: vec![],
        withdrawals: Some(attributes.payload_attributes.withdrawals),
        requests: Default::default(),
    }
    .with_recovered_senders()
    .ok_or(BlockExecutionError::Validation(BlockValidationError::SenderRecoveryError))?;

    // execute the block
    let BlockExecutionOutput { state, receipts, requests, gas_used } =
        executor.executor(&mut db).execute((&mut block, U256::ZERO).into())?;
    let execution_outcome =
        ExecutionOutcome::new(state, receipts.into(), block.number, vec![requests.into()]);

    // todo(onbjerg): we should not pass requests around as this is building a block, which
    // means we need to extract the requests from the execution output and compute the requests
    // root here

    // now we need to update certain header fields with the results of the execution
    block.header.transactions_root = proofs::calculate_transaction_root(&block.body);
    block.header.state_root = db.state_root(execution_outcome.state())?;
    block.header.gas_used = gas_used;

    let receipts = execution_outcome.receipts_by_block(block.header.number);

    // update logs bloom
    let receipts_with_bloom =
        receipts.iter().map(|r| r.as_ref().unwrap().bloom_slow()).collect::<Vec<Bloom>>();
    block.header.logs_bloom = receipts_with_bloom.iter().fold(Bloom::ZERO, |bloom, r| bloom | *r);

    // update receipts root
    block.header.receipts_root =
        execution_outcome.receipts_root_slow(block.header.number).expect("Receipts is present");

    let sealed_block = block.block.seal_slow();

    // L1Origin **MUST NOT** be nil, it's a required field in PayloadAttributesV1.
    let l1_origin = L1Origin {
        // Set the block hash before inserting the L1Origin into database.
        l2_block_hash: sealed_block.hash(),
        ..attributes.l1_origin
    };
    debug!(target: "taiko_payload_builder", ?l1_origin, "save l1 origin");
    let block_id = l1_origin.block_id.try_into().unwrap();
    // Write L1Origin and head L1Origin.
    client.save_l1_origin(block_id, l1_origin)?;

    debug!(target: "taiko_payload_builder", ?sealed_block, "sealed built block");

    let payload =
        TaikoBuiltPayload::new(attributes.payload_attributes.id, sealed_block, U256::ZERO);
    Ok(payload)
}
