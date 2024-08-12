use crate::Storage;
use futures_util::{future::BoxFuture, FutureExt};
use reth_chainspec::ChainSpec;
use reth_evm::execute::BlockExecutorProvider;
use reth_execution_errors::BlockExecutionError;
use reth_primitives::{Address, IntoRecoveredTransaction, TransactionSignedEcRecovered};
use reth_provider::{CanonChainTracker, StateProviderFactory};
use reth_transaction_pool::{TransactionPool, ValidPoolTransaction};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::{mpsc::UnboundedReceiver, oneshot};

pub struct TriggerArgs {
    pub beneficiary: Address,
    pub base_fee: u64,
    pub block_max_gas_limit: u64,
    pub max_bytes_per_tx_list: u64,
    pub local_accounts: Vec<Address>,
    pub max_transactions_lists: u64,
    pub min_tip: u64,
    pub tx: oneshot::Sender<Result<Vec<TriggerResult>, BlockExecutionError>>,
}

pub struct TriggerResult {
    pub txs: Vec<TransactionSignedEcRecovered>,
    pub estimated_gas_used: u64,
    pub bytes_length: u64,
}

/// A Future that listens for new ready transactions and puts new blocks into storage
pub struct MiningTask<Client, Pool: TransactionPool, Executor> {
    /// The configured chain spec
    chain_spec: Arc<ChainSpec>,
    /// The client used to interact with the state
    client: Client,
    /// Single active future that inserts a new block into `storage`
    insert_task: Option<BoxFuture<'static, ()>>,
    /// Shared storage to insert new blocks
    storage: Storage,
    /// Pool where transactions are stored
    pool: Pool,
    /// backlog of sets of transactions ready to be mined
    queued: VecDeque<(
        TriggerArgs,
        Vec<Arc<ValidPoolTransaction<<Pool as TransactionPool>::Transaction>>>,
    )>,
    /// The type used for block execution
    block_executor: Executor,
    trigger_args_rx: UnboundedReceiver<TriggerArgs>,
}

// === impl MiningTask ===

impl<Executor, Client, Pool: TransactionPool> MiningTask<Client, Pool, Executor> {
    /// Creates a new instance of the task
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        chain_spec: Arc<ChainSpec>,
        storage: Storage,
        client: Client,
        pool: Pool,
        block_executor: Executor,
        trigger_args_rx: UnboundedReceiver<TriggerArgs>,
    ) -> Self {
        Self {
            chain_spec,
            client,
            insert_task: None,
            storage,
            pool,
            queued: Default::default(),
            block_executor,
            trigger_args_rx,
        }
    }
}

impl<Executor, Client, Pool> Future for MiningTask<Client, Pool, Executor>
where
    Client: StateProviderFactory + CanonChainTracker + Clone + Unpin + 'static,
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

                // ready to queue in new insert task
                let storage = this.storage.clone();
                let (trigger_args, txs) = this.queued.pop_front().expect("not empty");

                let client = this.client.clone();
                let chain_spec = Arc::clone(&this.chain_spec);
                let executor = this.block_executor.clone();

                // Create the mining future that creates a block, notifies the engine that drives
                // the pipeline
                this.insert_task = Some(Box::pin(async move {
                    let mut storage = storage.write().await;

                    let txs: Vec<_> = txs
                        .into_iter()
                        .map(|tx| {
                            let recovered = tx.to_recovered_transaction();
                            recovered.into_signed()
                        })
                        .collect();
                    let ommers = vec![];

                    let TriggerArgs {
                        tx,
                        beneficiary,
                        block_max_gas_limit,
                        max_bytes_per_tx_list,
                        max_transactions_lists,
                        ..
                    } = trigger_args;
                    tx.send(storage.build_and_execute(
                        txs,
                        ommers,
                        &client,
                        chain_spec,
                        &executor,
                        beneficiary,
                        block_max_gas_limit,
                        max_bytes_per_tx_list,
                        max_transactions_lists,
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
    for MiningTask<Client, Pool, EvmConfig>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiningTask").finish_non_exhaustive()
    }
}
