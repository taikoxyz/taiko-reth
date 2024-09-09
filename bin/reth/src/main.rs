#![allow(missing_docs)]

// We use jemalloc for performance reasons.
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use reth::args::{DiscoveryArgs, NetworkArgs, RpcServerArgs};
use reth_chainspec::{ChainSpecBuilder, MAINNET};
use reth_consensus::Consensus;
use reth_db::{test_utils::TempDatabase, DatabaseEnv};
use reth_execution_types::Chain;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::{FullNodeTypesAdapter, PayloadBuilderAttributes};
use reth_node_builder::{components::Components, Node, NodeAdapter, NodeBuilder, NodeComponentsBuilder, NodeConfig, NodeHandle, RethFullAdapter};
use reth_node_ethereum::{node::EthereumAddOns, EthEvmConfig, EthExecutorProvider, EthereumNode};
use reth_primitives::{address, Address, SealedBlockWithSenders, TransactionSigned, B256};
use reth_provider::{providers::BlockchainProvider};
use reth_tasks::TaskManager;
use reth_transaction_pool::{blobstore::DiskFileBlobStore, CoinbaseTipOrdering, EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor};
use std::{sync::Arc};
use alloy_rlp::Decodable;
use reth::rpc::types::engine::PayloadAttributes;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};
use gwyneth::{GwynethNode, GwynethPayloadAttributes, GwynethPayloadBuilderAttributes};


fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {

        let tasks = TaskManager::current();
        let exec = tasks.executor();

        let network_config = NetworkArgs {
            discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
            ..NetworkArgs::default()
        };
        let network_config = NetworkArgs {
            discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
            ..NetworkArgs::default()
        };

        let chain_spec = ChainSpecBuilder::default()
                .chain(gwyneth::exex::CHAIN_ID.into())
                .genesis(serde_json::from_str(include_str!("../../../crates/ethereum/node/tests/assets/genesis.json")).unwrap())
                .cancun_activated()
                .build();

        let node_config = NodeConfig::test()
            .with_chain(chain_spec.clone())
            .with_network(network_config.clone())
            .with_unused_ports()
            .with_rpc(RpcServerArgs::default().with_unused_ports().with_http())
            .set_dev(true);

        let NodeHandle { node: eth_node, node_exit_future: _ } = NodeBuilder::new(node_config.clone())
            .testing_node(exec.clone())
            .node(EthereumNode::default())
            .launch()
            .await?;

        let NodeHandle { node: gwyneth_node, node_exit_future: _ } = NodeBuilder::new(node_config.clone())
            .testing_node(exec.clone())
            .node(GwynethNode::default())
            .launch()
            .await?;

        let handle = builder
            .node(EthereumNode::default())
            .install_exex("Rollup", move |ctx| async {
                Ok(gwyneth::exex::Rollup::new(ctx, gwyneth_node).await?.start())
            })
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}
