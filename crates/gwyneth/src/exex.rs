use std::{marker::PhantomData, sync::Arc, collections::VecDeque};

use alloy_rlp::Decodable;
use alloy_sol_types::{sol, SolEventInterface};
use reth_revm::database::EvmStateProvider;

use crate::{
    engine_api::EngineApiContext, GwynethEngineTypes, GwynethNode, GwynethPayloadAttributes,
    GwynethPayloadBuilderAttributes,
};
use reth_consensus::Consensus;
use reth_db::{test_utils::TempDatabase, DatabaseEnv};
use reth_ethereum_engine_primitives::EthPayloadAttributes;
use reth_evm_ethereum::EthEvmConfig;
use reth_execution_types::Chain;
use reth_exex::{ExExContext, ExExEvent};
use reth_node_api::{FullNodeTypesAdapter, PayloadBuilderAttributes};
use reth_node_builder::{components::Components, FullNode, NodeAdapter};
use reth_node_ethereum::{node::EthereumAddOns, EthExecutorProvider};
use reth_payload_builder::EthBuiltPayload;
use reth_primitives::{
    address, Address, SealedBlock, SealedBlockWithSenders, TransactionSigned, B256, U256,
};
use reth_provider::{
    providers::BlockchainProvider, CanonStateSubscriptions, DatabaseProviderFactory,
};
use reth_rpc_types::engine::PayloadStatusEnum;
use reth_transaction_pool::{
    blobstore::DiskFileBlobStore, CoinbaseTipOrdering, EthPooledTransaction,
    EthTransactionValidator, Pool, TransactionValidationTaskExecutor,
};
use RollupContract::{BlockProposed, RollupContractEvents};

const ROLLUP_CONTRACT_ADDRESS: Address = address!("9fCF7D13d10dEdF17d0f24C62f0cf4ED462f65b7");
pub const CHAIN_ID: u64 = 167010;
const INITIAL_TIMESTAMP: u64 = 1710338135;
const RING_BUFFER_SIZE: usize = 128;


#[derive(Clone, Debug)]
struct L1L2Mapping {
    l1_block: u64,
    l2_block: u64,
    l2_hash: B256,
}

pub type GwynethFullNode = FullNode<
    NodeAdapter<
        FullNodeTypesAdapter<
            GwynethNode,
            Arc<TempDatabase<DatabaseEnv>>,
            BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>,
        >,
        Components<
            FullNodeTypesAdapter<
                GwynethNode,
                Arc<TempDatabase<DatabaseEnv>>,
                BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>,
            >,
            Pool<
                TransactionValidationTaskExecutor<
                    EthTransactionValidator<
                        BlockchainProvider<Arc<TempDatabase<DatabaseEnv>>>,
                        EthPooledTransaction,
                    >,
                >,
                CoinbaseTipOrdering<EthPooledTransaction>,
                DiskFileBlobStore,
            >,
            EthEvmConfig,
            EthExecutorProvider,
            Arc<dyn Consensus>,
        >,
    >,
    EthereumAddOns,
>;

sol!(RollupContract, "TaikoL1.json");

pub struct Rollup<Node: reth_node_api::FullNodeComponents> {
    ctx: ExExContext<Node>,
    node: GwynethFullNode,
    engine_api: EngineApiContext<GwynethEngineTypes>,
    l1_l2_ring_buffer: VecDeque<L1L2Mapping>,
}

impl<Node: reth_node_api::FullNodeComponents> Rollup<Node> {
    pub async fn new(ctx: ExExContext<Node>, node: GwynethFullNode) -> eyre::Result<Self> {
        let engine_api = EngineApiContext {
            engine_api_client: node.auth_server_handle().http_client(),
            canonical_stream: node.provider.canonical_state_stream(),
            _marker: PhantomData::<GwynethEngineTypes>,
        };
        Ok(Self {
            ctx,
            node, 
            /* payload_event_stream, */
            engine_api,
            l1_l2_ring_buffer: VecDeque::with_capacity(RING_BUFFER_SIZE),
        })
    }

    pub async fn start(mut self) -> eyre::Result<()> {
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
    pub async fn commit(&mut self, chain: &Chain) -> eyre::Result<()> {
        let events = decode_chain_into_rollup_events(chain);
        for (block, _, event) in events {
            // TODO: Don't emit ProposeBlock event but directely
            //  read the function call RollupContractCalls to extract Txs
            // let _call = RollupContractCalls::abi_decode(tx.input(), true)?;

            if let RollupContractEvents::BlockProposed(BlockProposed {
                blockId: l2_block_number,
                meta,
            }) = event
            {
                println!("block_number: {:?}", l2_block_number);
                println!("tx_list: {:?}", meta.txList);
                let transactions: Vec<TransactionSigned> = decode_transactions(&meta.txList);
                println!("transactions: {:?}", transactions);

                let attrs: GwynethPayloadAttributes = GwynethPayloadAttributes {
                    inner: EthPayloadAttributes {
                        timestamp: INITIAL_TIMESTAMP,
                        prev_randao: B256::ZERO,
                        suggested_fee_recipient: Address::ZERO,
                        withdrawals: Some(vec![]),
                        parent_beacon_block_root: Some(B256::ZERO),
                    },
                    transactions: Some(transactions.clone()),
                    gas_limit: None,
                };

                let l1_state_provider = self
                    .ctx
                    .provider()
                    .database_provider_ro()
                    .unwrap()
                    .state_provider_by_block_number(block.number)
                    .unwrap();

                let l1_block_number = block.number;

                let mut builder_attrs =
                    GwynethPayloadBuilderAttributes::try_new(B256::ZERO, attrs).unwrap();
                builder_attrs.l1_provider =
                    Some((self.ctx.config.chain.chain().id(), Arc::new(l1_state_provider)));

                let payload_id = builder_attrs.inner.payload_id();
                let parrent_beacon_block_root =
                    builder_attrs.inner.parent_beacon_block_root.unwrap();
                // trigger new payload building draining the pool
                self.node.payload_builder.new_payload(builder_attrs).await.unwrap();
                // wait for the payload builder to have finished building
                let mut payload =
                    EthBuiltPayload::new(payload_id, SealedBlock::default(), U256::ZERO);
                loop {
                    payload =
                        self.node.payload_builder.best_payload(payload_id).await.unwrap().unwrap();
                    if payload.block().body.is_empty() {
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        continue;
                    }
                    break;
                }
                // trigger resolve payload via engine api
                self.engine_api.get_payload_v3_value(payload_id).await?;
                // submit payload to engine api
                let block_hash = self
                    .engine_api
                    .submit_payload(
                        payload.clone(),
                        parrent_beacon_block_root,
                        PayloadStatusEnum::Valid,
                        vec![],
                    )
                    .await?;

                 // Convert l2_block_number to u64 if necessary
                 let l2_block_u64 = l2_block_number.try_into().unwrap_or(u64::MAX);

                // Update the L1-L2 mapping in the ring buffer
                self.update_l1_l2_ring_buffer(l1_block_number, l2_block_u64, block_hash);

                // trigger forkchoice update via engine api to commit the block to the blockchain
                self.engine_api.update_forkchoice(block_hash, block_hash).await?;
            }
        }

        Ok(())
    }

    fn revert(&mut self, chain: &Chain) -> eyre::Result<()> {
        unimplemented!()
    }

    fn update_l1_l2_ring_buffer(&mut self, l1_block: u64, l2_block: u64, l2_hash: B256) {
        // Check if we already have an L2 enthy for this L1 block
        if let Some(existing_index) = self.l1_l2_ring_buffer.iter().position(|m| m.l1_block == l1_block) {
            // We have an existing entry, check if the new L2 block is higher
            let existing_mapping = &mut self.l1_l2_ring_buffer[existing_index];
            if l2_block > existing_mapping.l2_block {
                existing_mapping.l2_block = l2_block;
                existing_mapping.l2_hash = l2_hash;
            }
        } else {
            // No existing entry for this L1 block, add a new one
            let mapping = L1L2Mapping {
                l1_block,
                l2_block,
                l2_hash,
            };

            if self.l1_l2_ring_buffer.len() == RING_BUFFER_SIZE {
                // If the buffer is full, remove the oldest entry
                self.l1_l2_ring_buffer.pop_front();
            }

            // Add the new mapping to the end of the buffer
            self.l1_l2_ring_buffer.push_back(mapping);
        }
    }

    // New method to get the L2 block info for a given L1 block number
    pub fn get_l2_info_for_l1_block(&self, l1_block: u64) -> Option<(u64, B256)> {
        self.l1_l2_ring_buffer
            .iter()
            .find(|mapping| mapping.l1_block == l1_block)
            .map(|mapping| (mapping.l2_block, mapping.l2_hash))
    }
}

/// Decode chain of blocks into a flattened list of receipt logs, filter only transactions to the
/// Rollup contract [`ROLLUP_CONTRACT_ADDRESS`] and extract [`RollupContractEvents`].
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
                .filter(|log| {
                    log.address == ROLLUP_CONTRACT_ADDRESS
                })
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

pub fn decode_transactions(tx_list: &[u8]) -> Vec<TransactionSigned> {
    #[allow(clippy::useless_asref)]
    Vec::<TransactionSigned>::decode(&mut tx_list.as_ref()).unwrap_or_else(|e| {
        // If decoding fails we need to make an empty block
        println!("decode_transactions not successful: {e:?}, use empty tx_list");
        vec![]
    })
}
