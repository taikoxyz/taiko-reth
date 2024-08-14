use crate::{Storage, TaskArgs};
use futures_util::{future::BoxFuture, FutureExt};
use reth_chainspec::ChainSpec;
use reth_evm::execute::BlockExecutorProvider;
use reth_primitives::IntoRecoveredTransaction;
use reth_provider::{BlockReaderIdExt, StateProviderFactory};
use reth_transaction_pool::{TransactionPool, ValidPoolTransaction};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc::UnboundedReceiver;

/// A Future that listens for new ready transactions and puts new blocks into storage
pub struct ProposerTask<Provider, Pool: TransactionPool, Executor> {
    /// The configured chain spec
    chain_spec: Arc<ChainSpec>,
    /// The client used to interact with the state
    provider: Provider,
    /// Single active future that inserts a new block into `storage`
    insert_task: Option<BoxFuture<'static, ()>>,
    /// Pool where transactions are stored
    pool: Pool,
    /// backlog of sets of transactions ready to be mined
    #[allow(clippy::type_complexity)]
    queued: VecDeque<(
        TaskArgs,
        Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
    )>,
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
        Self {
            chain_spec,
            provider,
            insert_task: None,
            pool,
            queued: Default::default(),
            block_executor,
            trigger_args_rx,
        }
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
            if let Some(trigger_args) = match this.trigger_args_rx.poll_recv(cx) {
                Poll::Ready(Some(args)) => Some(args),
                Poll::Ready(None) => return Poll::Ready(()),
                _ => None,
            } {
                let mut best_txs = this.pool.best_transactions();
                best_txs.skip_blobs();
                let (mut local_txs, remote_txs): (Vec<_>, Vec<_>) = best_txs
                    .filter(|tx| {
                        tx.effective_tip_per_gas(trigger_args.base_fee)
                            .map_or(false, |tip| tip >= trigger_args.min_tip as u128)
                    })
                    .partition(|tx| trigger_args.local_accounts.contains(&tx.sender()));
                local_txs.extend(remote_txs);
                // miner returned a set of transaction that we feed to the producer
                this.queued.push_back((trigger_args, local_txs));
            };

            if this.insert_task.is_none() {
                if this.queued.is_empty() {
                    // nothing to insert
                    break;
                }

                // ready to queue in new insert task;
                let (trigger_args, txs) = this.queued.pop_front().expect("not empty");

                let client = this.provider.clone();
                let chain_spec = Arc::clone(&this.chain_spec);
                let executor = this.block_executor.clone();

                // Create the mining future that creates a block, notifies the engine that drives
                // the pipeline
                this.insert_task = Some(Box::pin(async move {
                    let txs: Vec<_> = txs
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
                    } = trigger_args;
                    let _ = tx.send(Storage::build_and_execute(
                        txs,
                        ommers,
                        &client,
                        chain_spec,
                        &executor,
                        beneficiary,
                        block_max_gas_limit,
                        max_bytes_per_tx_list,
                        max_transactions_lists,
                        base_fee,
                    ));
                }));
            }

            if let Some(mut fut) = this.insert_task.take() {
                match fut.poll_unpin(cx) {
                    Poll::Ready(_) => {}
                    Poll::Pending => {
                        this.insert_task = Some(fut);
                        break;
                    }
                }
            }
        }

        Poll::Pending
    }
}

impl<Client, Pool: TransactionPool, EvmConfig: std::fmt::Debug> std::fmt::Debug
    for ProposerTask<Client, Pool, EvmConfig>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiningTask").finish_non_exhaustive()
    }
}
