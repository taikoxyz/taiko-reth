//! A basic Ethereum payload builder implementation.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![allow(clippy::useless_let_if_seq)]

use reth_basic_payload_builder::{
    commit_withdrawals, is_better_payload, BuildArguments, BuildOutcome, PayloadConfig,
    WithdrawalsOutcome,
};
use reth_errors::RethError;
use reth_evm::{
    system_calls::{
        post_block_withdrawal_requests_contract_call, pre_block_beacon_root_contract_call,
    },
    ConfigureEvm,
};
use reth_evm_ethereum::eip6110::parse_deposits_from_receipts;
use reth_execution_types::ExecutionOutcome;
use reth_payload_builder::{error::PayloadBuilderError, EthBuiltPayload};
use reth_primitives::{
    constants::BEACON_NONCE,
    eip4844::calculate_excess_blob_gas,
    proofs::{self, calculate_requests_root},
    Block, EthereumHardforks, Header, Receipt, EMPTY_OMMER_ROOT_HASH, U256,
};
use reth_provider::StateProviderFactory;
use reth_revm::{database::StateProviderDatabase, state_change::apply_blockhashes_update};
use reth_transaction_pool::{BestTransactionsAttributes, TransactionPool};
use revm::{
    db::states::bundle_state::BundleRetention,
    primitives::{EVMError, EnvWithHandlerCfg, ResultAndState},
    DatabaseCommit, State,
};
use tracing::{debug, trace, warn};

use crate::GwynethPayloadBuilderAttributes;

/// Constructs an Ethereum transaction payload using the best transactions from the pool.
///
/// Given build arguments including an Ethereum client, transaction pool,
/// and configuration, this function creates a transaction payload. Returns
/// a result indicating success with the payload or an error in case of failure.
#[inline]
pub fn default_gwyneth_payload_builder<EvmConfig, Pool, Client, SP>(
    evm_config: EvmConfig,
    args: BuildArguments<Pool, Client, GwynethPayloadBuilderAttributes<SP>, EthBuiltPayload>,
) -> Result<BuildOutcome<EthBuiltPayload>, PayloadBuilderError>
where
    EvmConfig: ConfigureEvm,
    Client: StateProviderFactory,
    Pool: TransactionPool,
{
    // Brecht: ethereum payload builder

    let BuildArguments { client, pool, mut cached_reads, config, cancel, best_payload } = args;

    let state_provider = client.state_by_block_hash(config.parent_block.hash())?;
    let state = StateProviderDatabase::new(state_provider);
    let mut db =
        State::builder().with_database_ref(cached_reads.as_db(state)).with_bundle_update().build();
    let extra_data = config.extra_data();
    let PayloadConfig {
        initialized_block_env,
        initialized_cfg,
        parent_block,
        attributes,
        chain_spec,
        ..
    } = config;

    debug!(target: "payload_builder", id=%attributes.inner.id, parent_hash = ?parent_block.hash(), parent_number = parent_block.number, "building new payload");
    let mut cumulative_gas_used = 0;
    let mut sum_blob_gas_used = 0;
    let block_gas_limit: u64 =
        initialized_block_env.gas_limit.try_into().unwrap_or(chain_spec.max_gas_limit);
    let base_fee = initialized_block_env.basefee.to::<u64>();

    let mut executed_txs = Vec::new();

    let mut best_txs = pool.best_transactions_with_attributes(BestTransactionsAttributes::new(
        base_fee,
        initialized_block_env.get_blob_gasprice().map(|gasprice| gasprice as u64),
    ));

    let mut total_fees = U256::ZERO;

    let block_number = initialized_block_env.number.to::<u64>();

    println!("brecht: payload builder: {:?}", attributes.transactions);

    // apply eip-4788 pre block contract call
    pre_block_beacon_root_contract_call(
        &mut db,
        &evm_config,
        &chain_spec,
        &initialized_cfg,
        &initialized_block_env,
        block_number,
        attributes.inner.timestamp,
        attributes.inner.parent_beacon_block_root,
    )
    .map_err(|err| {
        warn!(target: "payload_builder",
            parent_hash=%parent_block.hash(),
            %err,
            "failed to apply beacon root contract call for empty payload"
        );
        PayloadBuilderError::Internal(err.into())
    })?;

    // apply eip-2935 blockhashes update
    apply_blockhashes_update(
        &mut db,
        &chain_spec,
        initialized_block_env.timestamp.to::<u64>(),
        block_number,
        parent_block.hash(),
    )
    .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

    let mut receipts = Vec::new();
    for sequencer_tx in &attributes.transactions {
        // Check if the job was cancelled, if so we can exit early.
        if cancel.is_cancelled() {
            return Ok(BuildOutcome::Cancelled)
        }

        let sequencer_tx = sequencer_tx.clone().1.try_into_ecrecovered().unwrap();

        let env = EnvWithHandlerCfg::new_with_cfg_env(
            initialized_cfg.clone(),
            initialized_block_env.clone(),
            evm_config.tx_env(&sequencer_tx),
        );

        let mut evm = evm_config.evm_with_env(&mut db, env);

        let ResultAndState { result, state } = match evm.transact() {
            Ok(res) => res,
            Err(err) => {
                match err {
                    EVMError::Transaction(err) => {
                        trace!(target: "payload_builder", %err, ?sequencer_tx, "Error in sequencer transaction, skipping.");
                        continue
                    }
                    err => {
                        // this is an error that we should treat as fatal for this attempt
                        return Err(PayloadBuilderError::EvmExecutionError(err))
                    }
                }
            }
        };

        // to release the db reference drop evm.
        drop(evm);
        // commit changes
        db.commit(state);

        let gas_used = result.gas_used();

        // add gas used by the transaction to cumulative gas used, before creating the receipt
        cumulative_gas_used += gas_used;

        // Push transaction changeset and calculate header bloom filter for receipt.
        receipts.push(Some(Receipt {
            tx_type: sequencer_tx.tx_type(),
            success: result.is_success(),
            cumulative_gas_used,
            logs: result.into_logs().into_iter().map(Into::into).collect(),
        }));

        // append transaction to the list of executed transactions
        executed_txs.push(sequencer_tx.into_signed());
    }
    // check if we have a better block
    if !is_better_payload(best_payload.as_ref(), total_fees) {
        // can skip building the block
        return Ok(BuildOutcome::Aborted { fees: total_fees, cached_reads })
    }

    // calculate the requests and the requests root
    let (requests, requests_root) = if chain_spec
        .is_prague_active_at_timestamp(attributes.inner.timestamp)
    {
        let deposit_requests = parse_deposits_from_receipts(&chain_spec, receipts.iter().flatten())
            .map_err(|err| PayloadBuilderError::Internal(RethError::Execution(err.into())))?;
        let withdrawal_requests = post_block_withdrawal_requests_contract_call(
            &evm_config,
            &mut db,
            &initialized_cfg,
            &initialized_block_env,
        )
        .map_err(|err| PayloadBuilderError::Internal(err.into()))?;

        let requests = [deposit_requests, withdrawal_requests].concat();
        let requests_root = calculate_requests_root(&requests);
        (Some(requests.into()), Some(requests_root))
    } else {
        (None, None)
    };

    let WithdrawalsOutcome { withdrawals_root, withdrawals } = commit_withdrawals(
        &mut db,
        &chain_spec,
        attributes.inner.timestamp,
        attributes.inner.withdrawals,
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
    let mut blob_sidecars = Vec::new();
    let mut excess_blob_gas = None;
    let mut blob_gas_used = None;

    // only determine cancun fields when active
    if chain_spec.is_cancun_active_at_timestamp(attributes.inner.timestamp) {
        // grab the blob sidecars from the executed txs
        blob_sidecars = pool.get_all_blobs_exact(
            executed_txs.iter().filter(|tx| tx.is_eip4844()).map(|tx| tx.hash).collect(),
        )?;

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
        timestamp: attributes.inner.timestamp,
        mix_hash: attributes.inner.prev_randao,
        nonce: BEACON_NONCE,
        base_fee_per_gas: Some(base_fee),
        number: parent_block.number + 1,
        gas_limit: block_gas_limit,
        difficulty: U256::ZERO,
        gas_used: cumulative_gas_used,
        extra_data,
        parent_beacon_block_root: attributes.inner.parent_beacon_block_root,
        blob_gas_used,
        excess_blob_gas,
        requests_root,
    };

    // seal the block
    let block = Block { header, body: executed_txs, ommers: vec![], withdrawals, requests };

    let sealed_block = block.seal_slow();
    debug!(target: "payload_builder", ?sealed_block, "sealed built block");

    let mut payload = EthBuiltPayload::new(attributes.inner.id, sealed_block, total_fees);

    // extend the payload with the blob sidecars from the executed txs
    payload.extend_sidecars(blob_sidecars);

    Ok(BuildOutcome::Better { payload, cached_reads })
}
