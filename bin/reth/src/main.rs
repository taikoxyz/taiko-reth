#![allow(missing_docs)]

// We use jemalloc for performance reasons.
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use gwyneth::{engine_api::RpcServerArgsExEx, GwynethNode};
use reth::args::{DiscoveryArgs, NetworkArgs, RpcServerArgs};
use reth_chainspec::ChainSpecBuilder;
use reth_node_builder::{NodeBuilder, NodeConfig, NodeHandle};
use reth_node_ethereum::EthereumNode;
use reth_tasks::TaskManager;

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let tasks = TaskManager::current();
        let exec = tasks.executor();

        let network_config = NetworkArgs {
            discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
            ..NetworkArgs::default()
        };

        let chain_spec = ChainSpecBuilder::default()
            .chain(gwyneth::exex::CHAIN_ID.into())
            .genesis(
                serde_json::from_str(include_str!(
                    "../../../crates/ethereum/node/tests/assets/genesis.json"
                ))
                .unwrap(),
            )
            .cancun_activated()
            .build();

        let node_config = NodeConfig::test()
            .with_chain(chain_spec.clone())
            .with_network(network_config.clone())
            .with_unused_ports()
            .with_rpc(RpcServerArgs::default().with_unused_ports().with_static_l2_rpc_ip_and_port(chain_spec.chain.id()));

        let NodeHandle { node: gwyneth_node, node_exit_future: _ } =
            NodeBuilder::new(node_config.clone())
                .gwyneth_node(exec.clone(), chain_spec.chain.id())
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, Parser};

    /// A helper type to parse Args more easily
    #[derive(Parser)]
    struct CommandParser<T: Args> {
        #[command(flatten)]
        args: T,
    }
}
