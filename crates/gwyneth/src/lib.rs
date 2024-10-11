//! Ethereum Node types config.
use std::{fmt::Debug, sync::Arc};

use builder::default_gwyneth_payload_builder;
use reth_evm_ethereum::EthEvmConfig;
use reth_primitives::ChainId;
use reth_tasks::TaskManager;
use thiserror::Error;

use reth_basic_payload_builder::{
    BasicPayloadJobGenerator, BasicPayloadJobGeneratorConfig, BuildArguments, BuildOutcome,
    PayloadBuilder, PayloadConfig,
};
use reth_chainspec::{Chain, ChainSpec};
use reth_ethereum_engine_primitives::{
    EthBuiltPayload, EthPayloadAttributes, EthPayloadBuilderAttributes, ExecutionPayloadEnvelopeV2,
    ExecutionPayloadEnvelopeV3, ExecutionPayloadEnvelopeV4,
};
use reth_node_api::{
    payload::{EngineApiMessageVersion, EngineObjectValidationError, PayloadOrAttributes},
    validate_version_specific_fields, EngineTypes, PayloadAttributes, PayloadBuilderAttributes,
};
use reth_node_builder::{
    components::{ComponentsBuilder, PayloadServiceBuilder},
    node::{FullNodeTypes, NodeTypes},
    BuilderContext, Node, NodeBuilder, NodeConfig, PayloadBuilderConfig, PayloadTypes,
};
use reth_node_core::{
    args::RpcServerArgs,
    primitives::{
        revm_primitives::{BlockEnv, CfgEnvWithHandlerCfg},
        transaction::WithEncoded,
        Address, Genesis, Header, TransactionSigned, Withdrawals, B256,
    },
};
use reth_node_ethereum::node::{
    EthereumAddOns, EthereumConsensusBuilder, EthereumExecutorBuilder, EthereumNetworkBuilder,
    EthereumPoolBuilder,
};
use reth_payload_builder::{
    error::PayloadBuilderError, PayloadBuilderHandle, PayloadBuilderService, PayloadId,
};
use reth_provider::{CanonStateSubscriptions, StateProviderBox, StateProviderFactory};
use reth_rpc_types::{ExecutionPayloadV1, Withdrawal};
use reth_tracing::{RethTracer, Tracer};
use reth_transaction_pool::TransactionPool;
use serde::{Deserialize, Serialize};

pub mod builder;
pub mod engine_api;
pub mod exex;

/// Gwyneth error type used in payload attributes validation
#[derive(Debug, Error)]
pub enum GwynetError {
    #[error("Gwyneth field is not zero")]
    RlpError(alloy_rlp::Error),
}

/// Gwyneth Payload Attributes
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GwynethPayloadAttributes {
    /// The payload attributes
    #[serde(flatten)]
    pub inner: EthPayloadAttributes,
    /// Transactions is a field for rollups: the transactions list is forced into the block
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transactions: Option<Vec<TransactionSigned>>,
    /// If set, this sets the exact gas limit the block produced with.
    #[serde(skip_serializing_if = "Option::is_none", with = "alloy_serde::quantity::opt")]
    pub gas_limit: Option<u64>,
}

impl PayloadAttributes for GwynethPayloadAttributes {
    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    fn withdrawals(&self) -> Option<&Vec<Withdrawal>> {
        self.inner.withdrawals.as_ref()
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.inner.parent_beacon_block_root
    }

    fn ensure_well_formed_attributes(
        &self,
        chain_spec: &ChainSpec,
        version: EngineApiMessageVersion,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(chain_spec, version, self.into())?;

        if self.gas_limit.is_none() {
            return Err(EngineObjectValidationError::InvalidParams(
                "MissingGasLimitInPayloadAttributes".to_string().into(),
            ));
        }

        Ok(())
    }
}

/// Gwyneth Payload Builder Attributes
#[derive(Clone, Debug)]
pub struct GwynethPayloadBuilderAttributes<SyncProvider> {
    /// Inner ethereum payload builder attributes
    pub inner: EthPayloadBuilderAttributes,
    /// Decoded transactions and the original EIP-2718 encoded bytes as received in the payload
    /// attributes.
    pub transactions: Vec<WithEncoded<TransactionSigned>>,
    /// The gas limit for the generated payload
    pub gas_limit: Option<u64>,

    pub l1_provider: Option<(ChainId, SyncProvider)>,
}

impl<SyncProvider: Debug + Sync + Send> PayloadBuilderAttributes
    for GwynethPayloadBuilderAttributes<SyncProvider>
{
    type RpcPayloadAttributes = GwynethPayloadAttributes;
    type Error = alloy_rlp::Error;

    fn try_new(
        parent: B256,
        attributes: GwynethPayloadAttributes,
    ) -> Result<Self, alloy_rlp::Error> {
        let transactions = attributes
            .transactions
            .unwrap_or_default()
            .into_iter()
            .map(|tx| WithEncoded::new(tx.envelope_encoded(), tx))
            .collect();

        Ok(Self {
            inner: EthPayloadBuilderAttributes::new(parent, attributes.inner),
            transactions,
            gas_limit: attributes.gas_limit,
            l1_provider: None,
        })
    }

    fn payload_id(&self) -> PayloadId {
        self.inner.id
    }

    fn parent(&self) -> B256 {
        self.inner.parent
    }

    fn timestamp(&self) -> u64 {
        self.inner.timestamp
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.inner.parent_beacon_block_root
    }

    fn suggested_fee_recipient(&self) -> Address {
        self.inner.suggested_fee_recipient
    }

    fn prev_randao(&self) -> B256 {
        self.inner.prev_randao
    }

    fn withdrawals(&self) -> &Withdrawals {
        &self.inner.withdrawals
    }

    fn cfg_and_block_env(
        &self,
        chain_spec: &ChainSpec,
        parent: &Header,
    ) -> (CfgEnvWithHandlerCfg, BlockEnv) {
        self.inner.cfg_and_block_env(chain_spec, parent)
    }
}

/// Gwyneth engine types - uses a Gwyneth payload attributes RPC type, but uses the default
/// payload builder attributes type.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GwynethEngineTypes;

impl PayloadTypes for GwynethEngineTypes {
    type BuiltPayload = EthBuiltPayload;
    type PayloadAttributes = GwynethPayloadAttributes;
    type PayloadBuilderAttributes = GwynethPayloadBuilderAttributes<Self::SyncProvider>;
    type SyncProvider = Arc<StateProviderBox>;
}

impl EngineTypes for GwynethEngineTypes {
    type ExecutionPayloadV1 = ExecutionPayloadV1;
    type ExecutionPayloadV2 = ExecutionPayloadEnvelopeV2;
    type ExecutionPayloadV3 = ExecutionPayloadEnvelopeV3;
    type ExecutionPayloadV4 = ExecutionPayloadEnvelopeV4;

    fn validate_version_specific_fields(
        chain_spec: &ChainSpec,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, GwynethPayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        validate_version_specific_fields(chain_spec, version, payload_or_attrs)
    }
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct GwynethNode;

/// Configure the node types
impl NodeTypes for GwynethNode {
    type Primitives = ();
    // use the Gwyneth engine types
    type Engine = GwynethEngineTypes;
    // use ethereum chain spec
    type ChainSpec = ChainSpec;
}

/// Implement the Node trait for the Gwyneth node
///
/// This provides a preset configuration for the node
impl<N> Node<N> for GwynethNode
where
    N: FullNodeTypes<Engine = GwynethEngineTypes>,
{
    type ComponentsBuilder = ComponentsBuilder<
        N,
        EthereumPoolBuilder,
        GwynethPayloadBuilder,
        EthereumNetworkBuilder,
        EthereumExecutorBuilder,
        EthereumConsensusBuilder,
    >;
    type AddOns = EthereumAddOns;

    fn components_builder(&self) -> Self::ComponentsBuilder {
        ComponentsBuilder::default()
            .node_types::<N>()
            .pool(EthereumPoolBuilder::default())
            .payload(GwynethPayloadBuilder::default())
            .network(EthereumNetworkBuilder::default())
            .executor(EthereumExecutorBuilder::default())
            .consensus(EthereumConsensusBuilder::default())
    }
}

/// The type responsible for building Gwyneth payloads
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct GwynethPayloadBuilder;

impl<Pool, Client> PayloadBuilder<Pool, Client> for GwynethPayloadBuilder
where
    Client: StateProviderFactory,
    Pool: TransactionPool,
{
    type Attributes = GwynethPayloadBuilderAttributes<Arc<StateProviderBox>>;
    type BuiltPayload = EthBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<Pool, Client, Self::Attributes, Self::BuiltPayload>,
    ) -> Result<BuildOutcome<Self::BuiltPayload>, PayloadBuilderError> {
        default_gwyneth_payload_builder(EthEvmConfig::default(), args)
    }

    fn build_empty_payload(
        &self,
        client: &Client,
        config: PayloadConfig<Self::Attributes>,
    ) -> Result<Self::BuiltPayload, PayloadBuilderError> {
        let PayloadConfig {
            initialized_block_env,
            initialized_cfg,
            parent_block,
            extra_data,
            attributes,
            chain_spec,
        } = config;
        let eth_payload_config = PayloadConfig {
            initialized_block_env,
            initialized_cfg,
            parent_block,
            extra_data,
            attributes: attributes.inner,
            chain_spec,
        };
        <reth_ethereum_payload_builder::EthereumPayloadBuilder as PayloadBuilder<Pool, Client>>::build_empty_payload(&reth_ethereum_payload_builder::EthereumPayloadBuilder::default(),client, eth_payload_config)
    }

    fn on_missing_payload(
        &self,
        _args: BuildArguments<Pool, Client, Self::Attributes, Self::BuiltPayload>,
    ) -> reth_basic_payload_builder::MissingPayloadBehaviour<Self::BuiltPayload> {
        reth_basic_payload_builder::MissingPayloadBehaviour::RaceEmptyPayload
    }
}

impl<Node, Pool> PayloadServiceBuilder<Node, Pool> for GwynethPayloadBuilder
where
    Node: FullNodeTypes<Engine = GwynethEngineTypes>,
    Pool: TransactionPool + Unpin + 'static,
{
    async fn spawn_payload_service(
        self,
        ctx: &BuilderContext<Node>,
        pool: Pool,
    ) -> eyre::Result<PayloadBuilderHandle<Node::Engine>> {
        let payload_builder = Self::default();
        let conf = ctx.payload_builder_config();

        let payload_job_config = BasicPayloadJobGeneratorConfig::default()
            .interval(conf.interval())
            .deadline(conf.deadline())
            .max_payload_tasks(conf.max_payload_tasks())
            .extradata(conf.extradata_bytes());

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

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let _guard = RethTracer::new().init()?;

    let tasks = TaskManager::current();

    // create gwyneth genesis with canyon at block 2
    let spec = ChainSpec::builder()
        .chain(Chain::mainnet())
        .genesis(Genesis::default())
        .london_activated()
        .paris_activated()
        .shanghai_activated()
        .build();

    // create node config
    let node_config =
        NodeConfig::test().with_rpc(RpcServerArgs::default().with_http()).with_chain(spec);

    let handle = NodeBuilder::new(node_config)
        .testing_node(tasks.executor())
        .launch_node(GwynethNode::default())
        .await
        .unwrap();

    handle.node_exit_future.await
}
