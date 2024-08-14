//! This includes download client implementations for auto sealing miners.

use crate::{TaskArgs, TaskResult};
use reth_errors::RethError;
use reth_primitives::Address;
use std::fmt::Debug;
use tokio::sync::{mpsc::UnboundedSender, oneshot};

/// A download client that polls the miner for transactions and assembles blocks to be returned in
/// the download process.
///
/// When polled, the miner will assemble blocks when miners produce ready transactions and store the
/// blocks in memory.
#[derive(Debug, Clone)]
pub struct ProposerClient {
    trigger_args_tx: UnboundedSender<TaskArgs>,
}

impl ProposerClient {
    pub(crate) const fn new(trigger_args_tx: UnboundedSender<TaskArgs>) -> Self {
        Self { trigger_args_tx }
    }

    /// get transactions from pool
    #[allow(clippy::too_many_arguments)]
    pub async fn tx_pool_content_with_min_tip(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Vec<Address>,
        max_transactions_lists: u64,
        min_tip: u64,
    ) -> Result<Vec<TaskResult>, RethError> {
        let (tx, rx) = oneshot::channel();
        self.trigger_args_tx
            .send(TaskArgs {
                beneficiary,
                base_fee,
                block_max_gas_limit,
                max_bytes_per_tx_list,
                local_accounts,
                max_transactions_lists,
                min_tip,
                tx,
            })
            .unwrap();
        rx.await.unwrap()
    }
}
