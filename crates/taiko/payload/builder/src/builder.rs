//! Taiko's payload builder module.

use crate::error::TaikoPayloadBuilderError;
use reth_basic_payload_builder::*;
use reth_errors::RethError;
use reth_evm::ConfigureEvm;
use reth_payload_builder::{database::CachedReads, error::PayloadBuilderError};
use reth_primitives::{
    constants::{eip4844::MAX_DATA_GAS_PER_BLOCK, BEACON_NONCE},
    eip4844::calculate_excess_blob_gas,
    proofs::{self, calculate_requests_root},
    revm::env::tx_env_with_recovered,
    Block, Header, Receipt, TransactionSigned, TransactionSignedEcRecovered, EMPTY_OMMER_ROOT_HASH,
    U256,
};
use reth_provider::{ExecutionOutcome, StateProviderFactory};
use reth_revm::{
    database::StateProviderDatabase,
    revm::{
        db::states::{bundle_state::BundleRetention, State},
        primitives::{EnvWithHandlerCfg, ResultAndState, TxEnv},
        DatabaseCommit, JournaledState,
    },
};
use reth_transaction_pool::TransactionPool;
use taiko_reth_beacon_consensus::{check_anchor_tx, decode_ontake_extra_data};
use taiko_reth_engine_primitives::{TaikoBuiltPayload, TaikoPayloadBuilderAttributes};
use taiko_reth_evm::{eip6110::parse_deposits_from_receipts, TaikoEvmConfig};
use taiko_reth_primitives::L1Origin;
use taiko_reth_provider::L1OriginWriter;
use tracing::{debug, trace};

/// Taiko's payload builder
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaikoPayloadBuilder<EvmConfig = TaikoEvmConfig> {
    /// The type responsible for creating the evm.
    evm_config: EvmConfig,
}

impl<EvmConfig> TaikoPayloadBuilder<EvmConfig> {
    /// `OptimismPayloadBuilder` constructor.
    pub const fn new(evm_config: EvmConfig) -> Self {
        Self { evm_config }
    }
}

impl Default for TaikoPayloadBuilder {
    fn default() -> Self {
        Self::new(TaikoEvmConfig::default())
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
        let mut cached_reads = Default::default();
        taiko_payload_builder(self.evm_config.clone(), client, &mut cached_reads, config)
    }
}

/// Constructs an Ethereum transaction payload using the best transactions from the pool.
///
/// Given build arguments including an Ethereum client, transaction pool,
/// and configuration, this function creates a transaction payload. Returns
/// a result indicating success with the payload or an error in case of failure.
#[inline]
fn taiko_payload_builder<EvmConfig, Client>(
    evm_config: EvmConfig,
    client: &Client,
    cached_reads: &mut CachedReads,
    config: PayloadConfig<TaikoPayloadBuilderAttributes>,
) -> Result<TaikoBuiltPayload, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm,
    Client: StateProviderFactory + L1OriginWriter,
{
    let state_provider = client.state_by_block_hash(config.parent_block.hash())?;
    let state = StateProviderDatabase::new(state_provider);
    let mut db =
        State::builder().with_database_ref(cached_reads.as_db(state)).with_bundle_update().build();
    let PayloadConfig {
        initialized_block_env,
        initialized_cfg,
        parent_block,
        attributes,
        chain_spec,
        ..
    } = config;

    debug!(target: "taiko_payload_builder", id=%attributes.payload_attributes.payload_id(), parent_hash = ?parent_block.hash(), parent_number = parent_block.number, "building new payload");
    let mut cumulative_gas_used = 0;
    let mut sum_blob_gas_used = 0;
    let block_gas_limit: u64 = initialized_block_env.gas_limit.try_into().unwrap_or(u64::MAX);
    let base_fee = initialized_block_env.basefee.to::<u64>();

    let mut executed_txs = Vec::new();

    let mut total_fees = U256::ZERO;

    let block_number = initialized_block_env.number.to::<u64>();

    // apply eip-4788 pre block contract call
    pre_block_beacon_root_contract_call(
        &mut db,
        &chain_spec,
        block_number,
        &initialized_cfg,
        &initialized_block_env,
        &attributes,
    )?;

    let transactions: Vec<TransactionSigned> =
        alloy_rlp::Decodable::decode(&mut attributes.block_metadata.tx_list.as_ref())
            .map_err(|_| PayloadBuilderError::other(TaikoPayloadBuilderError::FailedToDecodeTx))?;

    let mut receipts = Vec::new();
    for (idx, tx) in transactions.into_iter().enumerate() {
        let is_anchor = idx == 0;

        // ensure we still have capacity for this transaction
        if cumulative_gas_used + tx.gas_limit() > block_gas_limit {
            if is_anchor {
                return Err(PayloadBuilderError::other(
                    TaikoPayloadBuilderError::FailedToExecuteAnchor,
                ));
            }
            debug!(target: "taiko_payload_builder", want = %block_gas_limit, got = %(cumulative_gas_used + tx.gas_limit()), "skipping transaction because it would exceed the block gas limit");
            continue;
        }

        let tx = match tx.try_into_ecrecovered().map_err(|_| {
            PayloadBuilderError::other(TaikoPayloadBuilderError::TransactionEcRecoverFailed)
        }) {
            Ok(tx) => tx,
            Err(err) => {
                if is_anchor {
                    return Err(err);
                } else {
                    debug!(target: "taiko_payload_builder", error = ?err, "skipping transaction because it failed to recover");
                    continue;
                }
            }
        };

        if is_anchor {
            check_anchor_tx(
                &tx,
                tx.signer(),
                attributes.base_fee_per_gas.try_into().unwrap(),
                chain_spec.treasury(),
            )
            .map_err(|_| {
                PayloadBuilderError::other(TaikoPayloadBuilderError::InvalidAnchorTransaction)
            })?;
        }

        // the EIP-4844 can still fit in the block
        if let Some(blob_tx) = tx.transaction.as_eip4844() {
            let tx_blob_gas = blob_tx.blob_gas();
            if sum_blob_gas_used + tx_blob_gas > MAX_DATA_GAS_PER_BLOCK {
                // we can't fit this _blob_ transaction into the block, so we mark it as
                // invalid, which removes its dependent transactions from
                // the iterator. This is similar to the gas limit condition
                // for regular transactions above.
                trace!(target: "taiko_payload_builder", tx=?tx.hash, ?sum_blob_gas_used, ?tx_blob_gas, "skipping blob transaction because it would exceed the max data gas per block");
                // anchor tx can't be a blob tx
                continue;
            }
        }

        let taiko_tx_env_with_recovered = |tx: &TransactionSignedEcRecovered| -> TxEnv {
            let mut tx_env = tx_env_with_recovered(tx);
            tx_env.taiko.is_anchor = is_anchor;
            tx_env.taiko.treasury = chain_spec.treasury();
            if chain_spec.is_ontake_fork(block_number) {
                tx_env.taiko.basefee_ratio =
                    decode_ontake_extra_data(&attributes.block_metadata.extra_data);
            }
            tx_env
        };
        let env = EnvWithHandlerCfg::new_with_cfg_env(
            initialized_cfg.clone(),
            initialized_block_env.clone(),
            taiko_tx_env_with_recovered(&tx),
        );

        // Configure the environment for the block.
        let mut evm = evm_config.evm_with_env(&mut db, env);

        let ResultAndState { result, state } = match evm.transact() {
            Ok(res) => res,
            Err(err) => {
                if !is_anchor {
                    // Clear the state for the next tx
                    evm.context.evm.journaled_state = JournaledState::new(
                        evm.context.evm.journaled_state.spec,
                        Default::default(),
                    );
                    debug!(target: "taiko_payload_builder", error = ?err, "skipping transaction because it failed to execute");
                    continue;
                }
                return Err(PayloadBuilderError::EvmExecutionError(err));
            }
        };
        // drop evm so db is released.
        drop(evm);
        // commit changes
        db.commit(state);

        // add to the total blob gas used if the transaction successfully executed
        if let Some(blob_tx) = tx.transaction.as_eip4844() {
            let tx_blob_gas = blob_tx.blob_gas();
            sum_blob_gas_used += tx_blob_gas;
        }

        let gas_used = result.gas_used();

        // add gas used by the transaction to cumulative gas used, before creating the receipt
        cumulative_gas_used += gas_used;

        // Push transaction changeset and calculate header bloom filter for receipt.
        #[allow(clippy::needless_update)] // side-effect of optimism fields
        receipts.push(Some(Receipt {
            tx_type: tx.tx_type(),
            success: result.is_success(),
            cumulative_gas_used,
            logs: result.into_logs().into_iter().map(Into::into).collect(),
            ..Default::default()
        }));

        // update add to total fees
        let miner_fee = tx
            .effective_tip_per_gas(Some(base_fee))
            .expect("fee is always valid; execution succeeded");
        total_fees += U256::from(miner_fee) * U256::from(gas_used);

        // append transaction to the list of executed transactions
        executed_txs.push(tx.into_signed());
    }

    // calculate the requests and the requests root
    let (requests, requests_root) = if chain_spec
        .is_prague_active_at_timestamp(attributes.payload_attributes.timestamp)
    {
        let deposit_requests = parse_deposits_from_receipts(&chain_spec, receipts.iter().flatten())
            .map_err(|err| PayloadBuilderError::Internal(RethError::Execution(err.into())))?;
        let withdrawal_requests = post_block_withdrawal_requests_contract_call(
            &mut db,
            &initialized_cfg,
            &initialized_block_env,
        )?;

        let requests = [deposit_requests, withdrawal_requests].concat();
        let requests_root = calculate_requests_root(&requests);
        (Some(requests.into()), Some(requests_root))
    } else {
        (None, None)
    };

    let WithdrawalsOutcome { withdrawals_root, withdrawals } = commit_withdrawals(
        &mut db,
        &chain_spec,
        attributes.payload_attributes.timestamp,
        attributes.payload_attributes.withdrawals,
    )?;

    // merge all transitions into bundle state, this would apply the withdrawal balance changes
    // and 4788 contract call
    db.merge_transitions(BundleRetention::PlainState);

    let execution_outcome = ExecutionOutcome::new(
        db.take_bundle(),
        vec![receipts].into(),
        block_number,
        vec![requests.clone().unwrap_or_default()],
    );
    let receipts_root =
        execution_outcome.receipts_root_slow(block_number).expect("Number is in range");
    let logs_bloom = execution_outcome.block_logs_bloom(block_number).expect("Number is in range");

    // calculate the state root
    let state_root = {
        let state_provider = db.database.0.inner.borrow_mut();
        state_provider.db.state_root(execution_outcome.state())?
    };

    // create the block header
    let transactions_root = proofs::calculate_transaction_root(&executed_txs);

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

        blob_gas_used = Some(sum_blob_gas_used);
    }

    let header = Header {
        parent_hash: parent_block.hash(),
        ommers_hash: EMPTY_OMMER_ROOT_HASH,
        beneficiary: initialized_block_env.coinbase,
        state_root,
        transactions_root,
        receipts_root,
        withdrawals_root,
        logs_bloom,
        timestamp: attributes.payload_attributes.timestamp,
        mix_hash: initialized_block_env.prevrandao.unwrap(),
        nonce: BEACON_NONCE,
        base_fee_per_gas: Some(base_fee),
        number: block_number,
        gas_limit: block_gas_limit,
        difficulty: initialized_block_env.difficulty,
        gas_used: cumulative_gas_used,
        extra_data: attributes.block_metadata.extra_data.clone().into(),
        parent_beacon_block_root: attributes.payload_attributes.parent_beacon_block_root,
        blob_gas_used,
        excess_blob_gas,
        requests_root,
    };

    // seal the block
    let block = Block { header, body: executed_txs, ommers: vec![], withdrawals, requests };

    let sealed_block = block.seal_slow();

    // L1Origin **MUST NOT** be nil, it's a required field in PayloadAttributesV1.
    let l1_origin = L1Origin {
        // Set the block hash before inserting the L1Origin into database.
        l2_block_hash: sealed_block.hash(),
        ..attributes.l1_origin.clone()
    };
    debug!(target: "taiko_payload_builder", ?l1_origin, "save l1 origin");
    let block_id = l1_origin.block_id.try_into().unwrap();
    // Write L1Origin and head L1Origin.
    client.save_l1_origin(block_id, l1_origin)?;

    debug!(target: "taiko_payload_builder", ?sealed_block, "sealed built block");

    let payload =
        TaikoBuiltPayload::new(attributes.payload_attributes.id, sealed_block, total_fees);
    Ok(payload)
}
