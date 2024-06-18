//! Ethereum Node types config.

use crate::{TaikoEngineTypes, TaikoEvmConfig};
use reth_basic_payload_builder::{BasicPayloadJobGenerator, BasicPayloadJobGeneratorConfig};
use reth_network::NetworkHandle;
use reth_node_builder::{
    components::{ComponentsBuilder, NetworkBuilder, PayloadServiceBuilder, PoolBuilder},
    node::{FullNodeTypes, Node, NodeTypes},
    BuilderContext, PayloadBuilderConfig,
};
use reth_payload_builder::{PayloadBuilderHandle, PayloadBuilderService};
use reth_provider::CanonStateSubscriptions;
use reth_tracing::tracing::{debug, info};
use reth_transaction_pool::{
    blobstore::DiskFileBlobStore, EthTransactionPool, TransactionPool,
    TransactionValidationTaskExecutor,
};

/// Type configuration for a regular Ethereum node.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct TaikoNode;

impl TaikoNode {
    /// Creates a new instance of the Optimism node type.
    pub const fn new() -> Self {
        Self
    }

    /// Returns a [ComponentsBuilder] configured for a Taiko node.
    pub fn components<Node>(
    ) -> ComponentsBuilder<Node, TaikoPoolBuilder, TaikoPayloadBuilder, TaikoNetworkBuilder>
    where
        Node: FullNodeTypes<Engine = TaikoEngineTypes>,
    {
        ComponentsBuilder::default()
            .node_types::<Node>()
            .pool(TaikoPoolBuilder::default())
            .payload(TaikoPayloadBuilder::default())
            .network(TaikoNetworkBuilder::default())
    }
}

impl NodeTypes for TaikoNode {
    type Primitives = ();
    type Engine = TaikoEngineTypes;
    type Evm = TaikoEvmConfig;

    fn evm_config(&self) -> Self::Evm {
        TaikoEvmConfig::default()
    }
}

impl<N> Node<N> for TaikoNode
where
    N: FullNodeTypes<Engine = TaikoEngineTypes>,
{
    type PoolBuilder = TaikoPoolBuilder;
    type NetworkBuilder = TaikoNetworkBuilder;
    type PayloadBuilder = TaikoPayloadBuilder;

    fn components(
        self,
    ) -> ComponentsBuilder<N, Self::PoolBuilder, Self::PayloadBuilder, Self::NetworkBuilder> {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(TaikoPoolBuilder::default())
            .payload(TaikoPayloadBuilder::default())
            .network(TaikoNetworkBuilder::default())
    }
}

/// A basic Taiko transaction pool.
///
/// This contains various settings that can be configured and take precedence over the node's
/// config.
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct TaikoPoolBuilder {
    // TODO add options for txpool args
}

impl<Node> PoolBuilder<Node> for TaikoPoolBuilder
where
    Node: FullNodeTypes,
{
    type Pool = EthTransactionPool<Node::Provider, DiskFileBlobStore>;

    async fn build_pool(self, ctx: &BuilderContext<Node>) -> eyre::Result<Self::Pool> {
        let data_dir = ctx.data_dir();
        let blob_store = DiskFileBlobStore::open(data_dir.blobstore_path(), Default::default())?;
        let validator = TransactionValidationTaskExecutor::eth_builder(ctx.chain_spec())
            .with_head_timestamp(ctx.head().timestamp)
            .kzg_settings(ctx.kzg_settings()?)
            .with_additional_tasks(1)
            .build_with_tasks(
                ctx.provider().clone(),
                ctx.task_executor().clone(),
                blob_store.clone(),
            );

        let transaction_pool =
            reth_transaction_pool::Pool::eth_pool(validator, blob_store, ctx.pool_config());
        info!(target: "reth::cli", "Transaction pool initialized");
        let transactions_path = data_dir.txpool_transactions_path();

        // spawn txpool maintenance task
        {
            let pool = transaction_pool.clone();
            let chain_events = ctx.provider().canonical_state_stream();
            let client = ctx.provider().clone();
            let transactions_backup_config =
                reth_transaction_pool::maintain::LocalTransactionBackupConfig::with_local_txs_backup(transactions_path);

            ctx.task_executor().spawn_critical_with_graceful_shutdown_signal(
                "local transactions backup task",
                |shutdown| {
                    reth_transaction_pool::maintain::backup_local_transactions_task(
                        shutdown,
                        pool.clone(),
                        transactions_backup_config,
                    )
                },
            );

            // spawn the maintenance task
            ctx.task_executor().spawn_critical(
                "txpool maintenance task",
                reth_transaction_pool::maintain::maintain_transaction_pool_future(
                    client,
                    pool,
                    chain_events,
                    ctx.task_executor().clone(),
                    Default::default(),
                ),
            );
            debug!(target: "reth::cli", "Spawned txpool maintenance task");
        }

        Ok(transaction_pool)
    }
}

/// A Taiko payload service.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct TaikoPayloadBuilder;

impl<Node, Pool> PayloadServiceBuilder<Node, Pool> for TaikoPayloadBuilder
where
    Node: FullNodeTypes<Engine = TaikoEngineTypes>,
    Pool: TransactionPool + Unpin + 'static,
{
    async fn spawn_payload_service(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<PayloadBuilderHandle<Node::Engine>> {
        let payload_builder = reth_taiko_payload_builder::TaikoPayloadBuilder;
        let conf = ctx.payload_builder_config();

        let payload_job_config = BasicPayloadJobGeneratorConfig::default()
            .interval(conf.interval())
            .deadline(conf.deadline())
            .max_payload_tasks(conf.max_payload_tasks())
            .extradata(conf.extradata_rlp_bytes())
            .max_gas_limit(conf.max_gas_limit());

        let payload_generator = BasicPayloadJobGenerator::with_builder(
            ctx.provider().clone(),
            pool,
            ctx.task_executor().clone(),
            payload_job_config,
            ctx.chain_spec(),
            payload_builder,
        );
        let (payload_service, payload_builder) =
            PayloadBuilderService::new(payload_generator, ctx.provider().canonical_state_stream());

        ctx.task_executor().spawn_critical("payload builder service", Box::pin(payload_service));

        Ok(payload_builder)
    }
}

/// A basic Taiko network service.
#[derive(Debug, Default, Clone, Copy)]
pub struct TaikoNetworkBuilder {
    // TODO add closure to modify network
}

impl<Node, Pool> NetworkBuilder<Node, Pool> for TaikoNetworkBuilder
where
    Node: FullNodeTypes,
    Pool: TransactionPool + Unpin + 'static,
{
    async fn build_network(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<NetworkHandle> {
        let network = ctx.network_builder().await?;
        let handle = ctx.start_network(network, pool);

        Ok(handle)
    }
}
