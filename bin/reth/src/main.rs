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
use db::Database;
use execution::execute_block;
use once_cell::sync::Lazy;
use reth_chainspec::{ChainSpec, ChainSpecBuilder};
use reth_execution_types::Chain;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::FullNodeComponents;
use reth_node_ethereum::EthereumNode;
use reth_primitives::{address, Address, Genesis, SealedBlockWithSenders, TransactionSigned};
use reth_tracing::tracing::{error, info};
use rusqlite::Connection;
use std::sync::Arc;

mod db;
mod execution;

sol!(RollupContract, "TaikoL1.json");
use RollupContract::{BlockProposed, RollupContractCalls, RollupContractEvents};

const DATABASE_PATH: &str = "rollup.db";
const ROLLUP_CONTRACT_ADDRESS: Address = address!("9fCF7D13d10dEdF17d0f24C62f0cf4ED462f65b7");
const ROLLUP_SUBMITTER_ADDRESS: Address = address!("8943545177806ED17B9F23F0a21ee5948eCaa776");
const CHAIN_ID: u64 = 160011;
static CHAIN_SPEC: Lazy<Arc<ChainSpec>> = Lazy::new(|| {
    Arc::new(
        ChainSpecBuilder::default()
            .chain(CHAIN_ID.into())
            .genesis(Genesis::clique_genesis(CHAIN_ID, ROLLUP_SUBMITTER_ADDRESS))
            .shanghai_activated()
            .build(),
    )
});

struct Rollup<Node: FullNodeComponents> {
    ctx: ExExContext<Node>,
    db: Database,
}

impl<Node: FullNodeComponents> Rollup<Node> {
    fn new(ctx: ExExContext<Node>, connection: Connection) -> eyre::Result<Self> {
        let db = Database::new(connection)?;
        Ok(Self { ctx, db })
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

                    /*if let RollupContractCalls::submitBlock(RollupContract::submitBlockCall {
                        header,
                        blockData,
                        ..
                    }) = call
                    {*/
                        match execute_block(
                            &mut self.db,
                            self.ctx.pool(),
                            tx,
                            &block_metadata,
                            tx_list,
                            //blockDataHash,
                        )
                        .await
                        {
                            Ok((block, bundle, _, _)) => {
                                let block = block.seal_slow();
                                self.db.insert_block_with_bundle(&block, bundle)?;
                                info!(
                                    tx_hash = %tx.recalculate_hash(),
                                    chain_id = %CHAIN_ID,
                                    sequence = %block_metadata.l2BlockNumber,
                                    transactions = block.body.len(),
                                    "Block submitted, executed and inserted into database"
                                );
                            }
                            Err(err) => {
                                error!(
                                    %err,
                                    tx_hash = %tx.recalculate_hash(),
                                    chain_id = %CHAIN_ID,
                                    sequence = %block_metadata.l2BlockNumber,
                                    "Failed to execute block"
                                );
                            }
                        }
                    //}
                }
                // A deposit of ETH to the rollup contract. The deposit is added to the recipient's
                // balance and committed into the database.
                /*RollupContractEvents::Enter(RollupContract::Enter {
                    rollupChainId,
                    token,
                    rollupRecipient,
                    amount,
                }) => {
                    if rollupChainId != U256::from(CHAIN_ID) {
                        error!(tx_hash = %tx.recalculate_hash(), "Invalid rollup chain ID");
                        continue;
                    }
                    if token != Address::ZERO {
                        error!(tx_hash = %tx.recalculate_hash(), "Only ETH deposits are supported");
                        continue;
                    }

                    self.db.upsert_account(rollupRecipient, |account| {
                        let mut account = account.unwrap_or_default();
                        account.balance += amount;
                        Ok(account)
                    })?;

                    info!(
                        tx_hash = %tx.recalculate_hash(),
                        %amount,
                        recipient = %rollupRecipient,
                        "Deposit",
                    );
                }*/
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
                // The deposit is subtracted from the recipient's balance.
                RollupContractEvents::Enter(RollupContract::Enter {
                    rollupChainId,
                    token,
                    rollupRecipient,
                    amount,
                }) => {
                    if rollupChainId != U256::from(CHAIN_ID) {
                        error!(tx_hash = %tx.recalculate_hash(), "Invalid rollup chain ID");
                        continue;
                    }
                    if token != Address::ZERO {
                        error!(tx_hash = %tx.recalculate_hash(), "Only ETH deposits are supported");
                        continue;
                    }

                    self.db.upsert_account(rollupRecipient, |account| {
                        let mut account = account.ok_or(eyre::eyre!("account not found"))?;
                        account.balance -= amount;
                        Ok(account)
                    })?;

                    info!(
                        tx_hash = %tx.recalculate_hash(),
                        %amount,
                        recipient = %rollupRecipient,
                        "Deposit reverted",
                    );
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

fn main() -> eyre::Result<()> {
    println!("Brecht");
    reth::cli::Cli::parse_args().run(|builder, _| async move {
        let handle = builder
            .node(EthereumNode::default())
            .install_exex("Rollup", move |ctx| async {
                let connection = Connection::open(DATABASE_PATH)?;

                /*let network_config = NetworkArgs {
                    discovery: DiscoveryArgs { disable_discovery: true, ..DiscoveryArgs::default() },
                    ..NetworkArgs::default()
                };

                let tasks = TaskManager::current();
                let exec = tasks.executor();

                let node_config = NodeConfig::test()
                    .with_chain(CHAIN_SPEC.clone())
                    .with_network(network_config.clone())
                    .with_unused_ports()
                    .with_rpc(RpcServerArgs::default().with_unused_ports().with_http())
                    .set_dev(true);

                let NodeHandle { node, node_exit_future: _ } = NodeBuilder::new(node_config.clone())
                    .testing_node(exec.clone())
                    .node(Default::default())
                    .launch()
                    .await?;

                // setup payload for submission
                let envelope_v3: <E as EngineTypes>::ExecutionPayloadV3 = payload.into();

                // submit payload to engine api
                let submission = EngineApiClient::<E>::new_payload_v3(
                    &self.engine_api_client,
                    envelope_v3.execution_payload(),
                    versioned_hashes,
                    payload_builder_attributes.parent_beacon_block_root().unwrap(),
                )
                .await?;*/

                Ok(Rollup::new(ctx, connection)?.start())
            })
            .launch()
            .await?;

        handle.wait_for_node_exit().await
    })
}
