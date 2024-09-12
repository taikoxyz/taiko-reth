#![allow(missing_docs)]

// We use jemalloc for performance reasons.
#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/*#[cfg(all(feature = "optimism", not(test)))]
compile_error!("Cannot build the `reth` binary with the `optimism` feature flag enabled. Did you mean to build `op-reth`?");

#[cfg(not(feature = "optimism"))]
fn main() {
    use reth::cli::Cli;
    use reth_node_ethereum::EthereumNode;

    reth_cli_util::sigsegv_handler::install();

    // Enable backtraces unless a RUST_BACKTRACE value has already been explicitly provided.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    if let Err(err) = Cli::parse_args().run(|builder, _| async {
        let handle = builder.launch_node(EthereumNode::default()).await?;
        handle.node_exit_future.await
    }) {
        eprintln!("Error: {err:?}");
        std::process::exit(1);
    }
}*/

use alloy_sol_types::{sol, SolEventInterface, SolInterface};
use node::NodeTestContext;
use reth::args::{DiscoveryArgs, NetworkArgs, RpcServerArgs};
use reth_chainspec::{ChainSpec, ChainSpecBuilder, MAINNET};
use reth_consensus::Consensus;
use reth_db::{test_utils::TempDatabase, DatabaseEnv};
use reth_execution_types::Chain;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::{FullNodeTypesAdapter, NodeAddOns};
//use reth_node_api::{EngineTypes, FullNodeComponents, NodeAddOns};
use reth_node_builder::{components::Components, rpc::EthApiBuilderProvider, AddOns, FullNode, Node, NodeAdapter, NodeBuilder, NodeComponentsBuilder, NodeConfig, NodeHandle, RethFullAdapter};
use reth_node_ethereum::{node::EthereumAddOns, EthEvmConfig, EthExecutorProvider, EthereumNode};
use reth_primitives::{address, alloy_primitives, Address, Genesis, SealedBlockWithSenders, TransactionSigned, B256};
use reth_provider::providers::BlockchainProvider;
use reth_rpc_api::{eth::{helpers::AddDevSigners, FullEthApiServer}, EngineApiClient};
use reth_tasks::TaskManager;
use reth_transaction_pool::{blobstore::DiskFileBlobStore, CoinbaseTipOrdering, EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor};
use traits::RpcServerArgsExEx;
use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use alloy_rlp::Decodable;

use reth::rpc::types::engine::PayloadAttributes;
use reth_payload_builder::{EthBuiltPayload, EthPayloadBuilderAttributes};

mod network;
mod payload;
mod rpc;
mod node;
mod engine_api;
mod traits;

/// Helper function to create a new eth payload attributes
pub(crate) fn eth_payload_attributes(timestamp: u64, transactions: Vec<TransactionSigned>) -> EthPayloadBuilderAttributes {
    let attributes = PayloadAttributes {
        timestamp,
        prev_randao: B256::ZERO,
        suggested_fee_recipient: Address::ZERO,
        withdrawals: Some(vec![]),
        parent_beacon_block_root: Some(B256::ZERO),
    };
    EthPayloadBuilderAttributes::new(B256::ZERO, attributes, Some(transactions))
}


sol!(RollupContract, "TaikoL1.json");
use RollupContract::{BlockProposed, RollupContractCalls, RollupContractEvents};

const ROLLUP_CONTRACT_ADDRESS: Address = address!("9fCF7D13d10dEdF17d0f24C62f0cf4ED462f65b7");
const CHAIN_ID: u64 = 167010;

pub fn decode_transactions(tx_list: &[u8]) -> Vec<TransactionSigned> {
    #[allow(clippy::useless_asref)]
    Vec::<TransactionSigned>::decode(&mut tx_list.as_ref()).unwrap_or_else(|e| {
        // If decoding fails we need to make an empty block
        println!("decode_transactions not successful: {e:?}, use empty tx_list");
        vec![]
    })
}

struct Rollup<Node: reth_node_api::FullNodeComponents> {
    ctx: ExExContext<Node>,
    node: TestNodeContext,
}

impl<Node: reth_node_api::FullNodeComponents> Rollup<Node> {
    fn new(ctx: ExExContext<Node>, node: TestNodeContext) -> eyre::Result<Self> {
        Ok(Self { ctx, node })
    }

    async fn start(mut self) -> eyre::Result<()> {
        // Process all new chain state notifications
        while let Some(notification) = self.ctx.notifications.recv().await {
            if let Some(reverted_chain) = notification.reverted_chain() {
                self.revert(&reverted_chain)?;
            }

            if let Some(committed_chain) = notification.committed_chain() {
                self.commit(&committed_chain).await?;
                self.ctx.events.send(ExExEvent::FinishedHeight(committed_chain.tip().number))?;
            }
        }

        Ok(())
    }

    /// Process a new chain commit.
    ///
    /// This function decodes all transactions to the rollup contract into events, executes the
    /// corresponding actions and inserts the results into the database.
    async fn commit(&mut self, chain: &Chain) -> eyre::Result<()> {
        let events = decode_chain_into_rollup_events(chain);
        println!("Found {:?} events", events.len());

        for (_, tx, event) in events {
            match event {
                // A new block is submitted to the rollup contract.
                // The block is executed on top of existing rollup state and committed into the
                // database.
                RollupContractEvents::BlockProposed(BlockProposed {
                    blockId: block_number,
                    meta: block_metadata,
                    txList: tx_list,
                }) => {
                    println!("block_number: {:?}", block_number);
                    println!("tx_list: {:?}", tx_list);
                    let _call = RollupContractCalls::abi_decode(tx.input(), true)?;

                    let transactions: Vec<TransactionSigned> = decode_transactions(&tx_list);
                    self.node.payload.transactions = transactions.clone();
                    println!("transactions: {:?}", transactions);
            
                    // submit the block
                    println!("payload start");
                    let (payload, _): (EthBuiltPayload, _) = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on({
                            self.node.advance_block(vec![], eth_payload_attributes)
                        })
                    }).unwrap();
                    println!("payload end");

                    let block_hash = payload.block().hash();
                    let block_number = payload.block().number;
                    println!("block_hash: {:?}", block_hash);
                    println!("block_number: {:?}", block_number);

                    let res = tokio::task::block_in_place(|| {
                        tokio::runtime::Handle::current().block_on({
                            // do something async
                            println!("assert_new_block");
                            self.node.assert_new_block(transactions[0].hash(), block_hash, block_number)
                        })
                    });
                    println!("assert_new_block done: {:?}", res);
                }
                _ => (),
            }
        }

        Ok(())
    }

    /// Process a chain revert.
    ///
    /// This function decodes all transactions to the rollup contract into events, reverts the
    /// corresponding actions and updates the database.
    fn revert(&mut self, chain: &Chain) -> eyre::Result<()> {
        let mut events = decode_chain_into_rollup_events(chain);
        // Reverse the order of events to start reverting from the tip
        events.reverse();

        /*for (_, tx, event) in events {
            match event {
                // The block is reverted from the database.
                RollupContractEvents::BlockSubmitted(_) => {
                    let call = RollupContractCalls::abi_decode(tx.input(), true)?;

                    if let RollupContractCalls::submitBlock(RollupContract::submitBlockCall {
                        header,
                        ..
                    }) = call
                    {
                        self.db.revert_tip_block(header.sequence)?;
                        info!(
                            tx_hash = %tx.recalculate_hash(),
                            chain_id = %header.rollupChainId,
                            sequence = %header.sequence,
                            "Block reverted"
                        );
                    }
                }
                _ => (),
            }
        }*/

        Ok(())
    }
}

/// Decode chain of blocks into a flattened list of receipt logs, filter only transactions to the
/// Rollup contract [ROLLUP_CONTRACT_ADDRESS] and extract [RollupContractEvents].
fn decode_chain_into_rollup_events(
    chain: &Chain,
) -> Vec<(&SealedBlockWithSenders, &TransactionSigned, RollupContractEvents)> {
    chain
        // Get all blocks and receipts
        .blocks_and_receipts()
        // Get all receipts
        .flat_map(|(block, receipts)| {
            block
                .body
                .iter()
                .zip(receipts.iter().flatten())
                .map(move |(tx, receipt)| (block, tx, receipt))
        })
        // Get all logs from rollup contract
        .flat_map(|(block, tx, receipt)| {
            receipt
                .logs
                .iter()
                .filter(|log| { println!("log: {:?}", log); log.address == ROLLUP_CONTRACT_ADDRESS } )
                .map(move |log| (block, tx, log))
        })
        // Decode and filter rollup events
        .filter_map(|(block, tx, log)| {
            RollupContractEvents::decode_raw_log(log.topics(), &log.data.data, true)
                .ok()
                .map(|event| (block, tx, event))
        })
        .collect()
}

// Type aliases

type TmpDB = Arc<TempDatabase<DatabaseEnv>>;
type TmpNodeAdapter<N> = FullNodeTypesAdapter<N, TmpDB, BlockchainProvider<TmpDB>>;

type Adapter<N> = NodeAdapter<
    RethFullAdapter<TmpDB, N>,
    <<N as Node<TmpNodeAdapter<N>>>::ComponentsBuilder as NodeComponentsBuilder<
        RethFullAdapter<TmpDB, N>,
    >>::Components,
>;

type TestNodeContext = NodeTestContext<NodeAdapter<FullNodeTypesAdapter<EthereumNode, Arc<TempDatabase<DatabaseEnv>>, BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>>, Components<FullNodeTypesAdapter<EthereumNode, Arc<TempDatabase<DatabaseEnv>>, BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>>, Pool<TransactionValidationTaskExecutor<EthTransactionValidator<BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>, EthPooledTransaction>>, CoinbaseTipOrdering<EthPooledTransaction>, DiskFileBlobStore>, EthEvmConfig, EthExecutorProvider, Arc<dyn Consensus>>>, EthereumAddOns>;

/// Type alias for a type of NodeHelper
pub type NodeHelperType<N, AO> = NodeTestContext<Adapter<N>, AO>;

fn main() -> eyre::Result<()> {
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        // Create the L2 node
        let tasks = TaskManager::current();
        let exec = tasks.executor();

        let network_config = NetworkArgs {
            discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
            ..NetworkArgs::default()
        };

        let chain_spec = ChainSpecBuilder::default()
                .chain(CHAIN_ID.into())
                .genesis(serde_json::from_str(include_str!("../../../crates/ethereum/node/tests/assets/genesis.json")).unwrap())
                .cancun_activated()
                .build();

        let node_config = NodeConfig::test()
            .with_chain(chain_spec.clone())
            .with_network(network_config.clone())
            .with_unused_ports()
            .with_rpc(RpcServerArgs::default().with_unused_ports().with_static_l2_rpc_ip_and_port());
            // .set_dev(true);

        let NodeHandle { node, node_exit_future: _ } = NodeBuilder::new(node_config.clone())
            .testing_node(exec.clone())
            .node(Default::default())
            .launch()
            .await?;

        let node = NodeTestContext::new(node).await?;

        let handle = builder
            .node(EthereumNode::default())
            .install_exex("Rollup", move |ctx| async {
                Ok(Rollup::new(ctx, node)?.start())
            })
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}
