use crate::{Storage, TaskArgs};
use reth_chainspec::ChainSpec;
use reth_evm::execute::BlockExecutorProvider;
use reth_primitives::IntoRecoveredTransaction;
use reth_provider::{BlockReaderIdExt, StateProviderFactory};
use reth_transaction_pool::TransactionPool;
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::debug;

/// A Future that listens for new ready transactions and puts new blocks into storage
pub struct ProposerTask<Provider, Pool: TransactionPool, Executor> {
    /// The configured chain spec
    chain_spec: Arc<ChainSpec>,
    /// The client used to interact with the state
    provider: Provider,
    /// Pool where transactions are stored
    pool: Pool,
    /// The type used for block execution
    block_executor: Executor,
    trigger_args_rx: UnboundedReceiver<TaskArgs>,
}

// === impl MiningTask ===

impl<Executor, Provider, Pool: TransactionPool> ProposerTask<Provider, Pool, Executor> {
    /// Creates a new instance of the task
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        chain_spec: Arc<ChainSpec>,
        provider: Provider,
        pool: Pool,
        block_executor: Executor,
        trigger_args_rx: UnboundedReceiver<TaskArgs>,
    ) -> Self {
        Self { chain_spec, provider, pool, block_executor, trigger_args_rx }
    }
}

impl<Executor, Provider, Pool> Future for ProposerTask<Provider, Pool, Executor>
where
    Provider: StateProviderFactory + BlockReaderIdExt + Clone + Unpin + 'static,
    Pool: TransactionPool + Unpin + 'static,
    <Pool as TransactionPool>::Transaction: IntoRecoveredTransaction,
    Executor: BlockExecutorProvider,
{
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // this drives block production and
        loop {
            match this.trigger_args_rx.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Ready(Some(args)) => {
                    let mut best_txs = this.pool.best_transactions();
                    best_txs.skip_blobs();
                    debug!(target: "taiko::proposer", txs = ?best_txs.size_hint(), "Proposer get best transactions");
                    let (mut local_txs, remote_txs): (Vec<_>, Vec<_>) = best_txs
                        .filter(|tx| {
                            tx.effective_tip_per_gas(args.base_fee)
                                .map_or(false, |tip| tip >= args.min_tip as u128)
                        })
                        .partition(|tx| {
                            args.local_accounts
                                .as_ref()
                                .map(|local_accounts| local_accounts.contains(&tx.sender()))
                                .unwrap_or_default()
                        });
                    local_txs.extend(remote_txs);
                    debug!(target: "taiko::proposer", txs = ?local_txs.len(), "Proposer filter best transactions");

                    let client = this.provider.clone();
                    let chain_spec = Arc::clone(&this.chain_spec);
                    let executor = this.block_executor.clone();

                    let txs: Vec<_> = local_txs
                        .into_iter()
                        .map(|tx| tx.to_recovered_transaction().into_signed())
                        .collect();
                    let ommers = vec![];

                    let TaskArgs {
                        tx,
                        beneficiary,
                        block_max_gas_limit,
                        max_bytes_per_tx_list,
                        max_transactions_lists,
                        base_fee,
                        ..
                    } = args;

                    let res = Storage::build_and_execute(
                        txs.clone(),
                        ommers,
                        &client,
                        chain_spec,
                        &executor,
                        beneficiary,
                        block_max_gas_limit,
                        max_bytes_per_tx_list,
                        max_transactions_lists,
                        base_fee,
                    );
                    if res.is_ok() {
                        let _ = tx.send(res);
                    }
                }
            }
        }
    }
}

impl<Client, Pool: TransactionPool, EvmConfig: std::fmt::Debug> std::fmt::Debug
    for ProposerTask<Client, Pool, EvmConfig>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiningTask").finish_non_exhaustive()
    }
}
