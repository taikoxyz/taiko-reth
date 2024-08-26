use crate::result::ToRpcResult;
use alloy_primitives::Address;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use reth_evm::execute::BlockExecutorProvider;
use reth_primitives::IntoRecoveredTransaction;
use reth_provider::{
    BlockNumReader, BlockReaderIdExt, ChainSpecProvider, EvmEnvProvider, StateProviderFactory,
};
use reth_rpc_api::{PreBuiltTxList, TaikoApiServer, TaikoAuthApiServer};
use reth_tasks::TaskSpawner;
use reth_transaction_pool::TransactionPool;
use taiko_reth_primitives::L1Origin;
use taiko_reth_proposer_consensus::{ProposerBuilder, ProposerClient};
use taiko_reth_provider::L1OriginReader;

/// Taiko API.
#[derive(Debug)]
pub struct TaikoAuthApi<Provider, Pool, BlockExecutor> {
    proposer_client: ProposerClient,
    _marker: std::marker::PhantomData<(Provider, Pool, BlockExecutor)>,
}

impl<Provider, Pool, BlockExecutor> TaikoAuthApi<Provider, Pool, BlockExecutor>
where
    Provider: StateProviderFactory + BlockReaderIdExt + ChainSpecProvider + Clone + Unpin + 'static,
    Pool: TransactionPool + Unpin + 'static,
    <Pool as TransactionPool>::Transaction: IntoRecoveredTransaction,
    BlockExecutor: BlockExecutorProvider,
{
    /// Creates a new instance of `Taiko`.
    pub fn new(
        provider: Provider,
        pool: Pool,
        block_executor: BlockExecutor,
        task_spawner: Box<dyn TaskSpawner>,
    ) -> Self {
        let chain_spec = provider.chain_spec();
        let (_, proposer_client, proposer_task) =
            ProposerBuilder::new(chain_spec, provider, pool, block_executor).build();
        task_spawner.spawn(Box::pin(proposer_task));

        Self { proposer_client, _marker: Default::default() }
    }
}
/// Taiko API
#[derive(Debug)]
pub struct TaikoApi<Provider> {
    provider: Provider,
}

impl<Provider> TaikoApi<Provider> {
    /// Creates a new instance of `Taiko`.
    pub const fn new(provider: Provider) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<Provider> TaikoApiServer for TaikoApi<Provider>
where
    Provider: L1OriginReader + 'static,
{
    /// HeadL1Origin returns the latest L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn head_l1_origin(&self) -> RpcResult<L1Origin> {
        self.provider.get_head_l1_origin().to_rpc_result()
    }

    /// L1OriginByID returns the L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn l1_origin_by_id(&self, block_id: u64) -> RpcResult<L1Origin> {
        self.provider.get_l1_origin(block_id).to_rpc_result()
    }
}

#[async_trait]
impl<Provider, Pool, BlockExecutor> TaikoAuthApiServer
    for TaikoAuthApi<Provider, Pool, BlockExecutor>
where
    Provider: StateProviderFactory + ChainSpecProvider + EvmEnvProvider + BlockNumReader + 'static,
    Pool: TransactionPool + 'static,
    BlockExecutor: Send + Sync + 'static,
{
    /// TxPoolContent retrieves the transaction pool content with the given upper limits.
    async fn tx_pool_content(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Option<Vec<Address>>,
        max_transactions_lists: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>> {
        self.tx_pool_content_with_min_tip(
            beneficiary,
            base_fee,
            block_max_gas_limit,
            max_bytes_per_tx_list,
            local_accounts,
            max_transactions_lists,
            0,
        )
        .await
    }

    /// TxPoolContent retrieves the transaction pool content with the given upper limits.
    async fn tx_pool_content_with_min_tip(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Option<Vec<Address>>,
        max_transactions_lists: u64,
        min_tip: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>> {
        self.proposer_client
            .tx_pool_content_with_min_tip(
                beneficiary,
                base_fee,
                block_max_gas_limit,
                max_bytes_per_tx_list,
                local_accounts,
                max_transactions_lists,
                min_tip,
            )
            .await
            .map(|tx_lists| {
                tx_lists
                    .into_iter()
                    .map(|tx_list| PreBuiltTxList {
                        tx_list: tx_list.txs,
                        estimated_gas_used: tx_list.estimated_gas_used,
                        bytes_length: tx_list.bytes_length,
                    })
                    .collect()
            })
            .to_rpc_result()
    }
}
