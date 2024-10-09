use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use alloy_rlp::Decodable;
use ef_tests::assert::assert_equal;
use ef_tests::cases::blockchain_test::{should_skip, BlockchainTestCase};
use ef_tests::models::ForkSpec;
use ef_tests::result::assert_tests_pass;
use ef_tests::suite::find_all_files_with_extension;
use ef_tests::{Cases, Suite};
use ef_tests::{models::BlockchainTest, Case, Error};
use gwyneth::exex::INITIAL_TIMESTAMP;
use gwyneth::{GwynethPayloadAttributes, GwynethPayloadBuilder, GwynethPayloadBuilderAttributes};
use rayon::iter::{ParallelBridge, ParallelIterator};
use reth_basic_payload_builder::{BuildArguments, BuildOutcome, Cancelled, PayloadBuilder, PayloadConfig};
use reth_blockchain_tree::noop::NoopBlockchainTree;
use reth_chainspec::{ChainSpec, ChainSpecBuilder};
use reth_db::Database;
use reth_db_common::init::init_genesis;
use reth_ethereum_engine_primitives::EthPayloadAttributes;
use reth_node_api::PayloadBuilderAttributes;
use reth_node_core::cli;
use reth_payload_builder::database::CachedReads;
use reth_payload_builder::EthBuiltPayload;
use reth_primitives::{keccak256, Address, Bytes, StaticFileSegment, TransactionSigned, B256};
use reth_primitives::{BlockBody, SealedBlock};
use reth_provider::providers::BlockchainProvider;
use reth_provider::test_utils::create_test_provider_factory_with_chain_spec;
use reth_provider::{BlockReader, HashingWriter, HeaderProvider, ProviderFactory, StateProviderFactory};
use reth_provider::StaticFileWriter;
use reth_revm::database::{StateProviderDatabase, SyncEvmStateProvider, SyncStateProviderDatabase};
use reth_stages::{stages::ExecutionStage, ExecInput, Stage};
use reth_transaction_pool::noop::NoopTransactionPool;
use revm::primitives::ChainAddress;
use revm::SyncDatabase;


/// A handler for the blockchain test suite.
#[derive(Debug)]
pub struct SyncBlockchainTests {
    suite: String,
}

impl SyncBlockchainTests {
    /// Create a new handler for a subset of the blockchain test suite.
    pub const fn new(suite: String) -> Self {
        Self { suite }
    }
}

impl Suite for SyncBlockchainTests {
    type Case = SyncBlockchainTestCase;

    fn suite_name(&self) -> String {
        format!("BlockchainTests/{}", self.suite)
    }

    fn load(&self) -> (PathBuf, Cases<Self::Case>) {
        // Build the path to the test suite directory
        let suite_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testing/ef-tests/ethereum-tests")
            .join(self.suite_name());
        println!("{:?}", suite_path);

        // Verify that the path exists
        assert!(suite_path.exists(), "Test suite path does not exist: {suite_path:?}");

        // Find all files with the ".json" extension in the test suite directory
        let test_cases = find_all_files_with_extension(&suite_path, ".json")
            .into_iter()
            .map(|test_case_path| {
                let case = Self::Case::load(&test_case_path).expect("test case should load");
                (test_case_path, case)
            })
            .collect();

        (suite_path, Cases { test_cases })
    }
}

/// An Ethereum blockchain test.
#[derive(Debug, PartialEq, Eq)]
pub struct SyncBlockchainTestCase {
    tests: BTreeMap<String, BlockchainTest>,
    l2_payload: Vec<TransactionSigned>,
    skip: bool,
}

impl Case for SyncBlockchainTestCase {
    fn load(path: &Path) -> Result<Self, Error> {
        Ok(Self {
            tests: {
                let s = fs::read_to_string(path)
                    .map_err(|error| Error::Io { path: path.into(), error })?;
                serde_json::from_str(&s)
                    .map_err(|error| Error::CouldNotDeserialize { path: path.into(), error })?
            },
            l2_payload: Vec::new(),
            skip: should_skip(path),
        })
    }

    fn load_l2_payload(&mut self, l2_payload: Vec<TransactionSigned>) {
        self.l2_payload = l2_payload;
    }

    fn run(&self) -> Result<(), ef_tests::Error> {
        // If the test is marked for skipping, return a Skipped error immediately.
        if self.skip {
            return Err(Error::Skipped)
        }

        // Iterate through test cases, filtering by the network type to exclude specific forks.
        self.tests
            .values()
            .filter(|case| {
                !matches!(
                    case.network,
                    ForkSpec::ByzantiumToConstantinopleAt5 |
                        ForkSpec::Constantinople |
                        ForkSpec::ConstantinopleFix |
                        ForkSpec::MergeEOF |
                        ForkSpec::MergeMeterInitCode |
                        ForkSpec::MergePush0 |
                        ForkSpec::Unknown
                )
            })
            .par_bridge()
            .try_for_each(|case| {
                let l1_spec: Arc<reth_chainspec::ChainSpec> = Arc::new(case.network.clone().into());
                let l1_factory = create_test_provider_factory_with_chain_spec(l1_spec.clone());
                let last_block = execute_l1_case_and_commit(&l1_factory, case)?;

                let l2_spec = Arc::new(l2_chain_spec());
                let l2_factory = create_test_provider_factory_with_chain_spec(l2_spec.clone());

                let mut sync_state = SyncStateProviderDatabase::new(
                    Some(l1_spec.chain().id()), 
                    StateProviderDatabase::new(l1_factory.latest()?)
                );
                sync_state.add_db(
                    l2_spec.chain().id(), 
                    StateProviderDatabase::new(l2_factory.latest()?)
                );

                // Validate the post-state for the test case.
                match (&case.post_state, &case.post_state_hash) {
                    (Some(state), None) => {
                        // Validate accounts in the state against the provider's database.
                        for (&address, account) in state {
                            let res = sync_state.basic_account(ChainAddress(l1_spec.chain().id(), address))?.ok_or_else(|| {
                                Error::Assertion(format!("Expected account ({address}) is missing from DB: {self:?}"))
                            })?;

                            assert_equal(res.balance, account.balance, "Balance does not match")?;
                            assert_equal(res.nonce, account.nonce.to(), "Nonce does not match")?;

                            if let Some(bytecode_hash) = res.bytecode_hash {
                                assert_equal(keccak256(&account.code), bytecode_hash, "Bytecode does not match")?;
                            } else {
                                assert_equal(
                                    account.code.is_empty(),
                                    true,
                                    "Expected empty bytecode, got bytecode in db.",
                                )?;
                            }
                            for (slot, value) in &account.storage {
                                if let Ok(res) = 
                                    sync_state.storage(ChainAddress(l1_spec.chain().id(), address), B256::new(slot.to_be_bytes()))
                                {
                                    assert_equal(res.unwrap(), *value,&format!("Storage for slot {slot:?} does not match"))?;
                                } else {
                                    println!("# account {:?} addr {:?}", account, address);
                                    return Err(Error::Assertion(format!(
                                        "Slot {slot:?} is missing from the database. Expected {value:?}"
                                    )))
                                }
                            }
                        }
                    }
                    (None, Some(expected_state_root)) => {
                        // Insert state hashes into the provider based on the expected state root.
                        let last_block = last_block.unwrap_or_default();
                        l1_factory.provider_rw().unwrap().insert_hashes(
                            0..=last_block.number,
                            last_block.hash(),
                            *expected_state_root,
                        )?;
                    }
                    _ => return Err(Error::MissingPostState),
                }

                Result::<(), Error>::Ok(())
            })?;
        Ok(())
    }

    fn run_l2(&self) -> Result<(), Error> {
        // If the test is marked for skipping, return a Skipped error immediately.
        if self.skip {
            return Err(Error::Skipped)
        }

        // Iterate through test cases, filtering by the network type to exclude specific forks.
        self.tests
            .values()
            .filter(|case| {
                !matches!(
                    case.network,
                    ForkSpec::ByzantiumToConstantinopleAt5 |
                        ForkSpec::Constantinople |
                        ForkSpec::ConstantinopleFix |
                        ForkSpec::MergeEOF |
                        ForkSpec::MergeMeterInitCode |
                        ForkSpec::MergePush0 |
                        ForkSpec::Unknown
                )
            })
            .par_bridge()
            .try_for_each(|case| {
                // Create a new test database and initialize a provider for the test case.
                let l1_spec: Arc<reth_chainspec::ChainSpec> = Arc::new(case.network.clone().into());
                let l1_factory = create_test_provider_factory_with_chain_spec(l1_spec.clone());
                let _ = execute_l1_case_and_commit(&l1_factory, case)?;

                let l2_spec = Arc::new(l2_chain_spec());
                let l2_factory = create_test_provider_factory_with_chain_spec(l2_spec.clone());
                let genesis_hash = init_genesis(l2_factory.clone())
                    .map_err(|e| Error::Database(reth_db::DatabaseError::Other(e.to_string())))?;
                let blockchain_db = BlockchainProvider::new(
                    l2_factory.clone(),
                    Arc::new(NoopBlockchainTree::default()),
                )?;
                let l2_genesis_block = blockchain_db.block_by_hash(genesis_hash).unwrap().unwrap();

                let attrs = GwynethPayloadAttributes {
                    inner: EthPayloadAttributes {
                        timestamp: INITIAL_TIMESTAMP,
                        prev_randao: B256::ZERO,
                        suggested_fee_recipient: Address::ZERO,
                        withdrawals: Some(vec![]),
                        parent_beacon_block_root: Some(B256::ZERO),
                    },
                    transactions: Some(self.l2_payload.clone()),
                    gas_limit: None,
                };
                
                let mut builder_attrs = GwynethPayloadBuilderAttributes::try_new(B256::ZERO, attrs).unwrap();
                builder_attrs.l1_provider = Some((l1_spec.chain().id(), Arc::new(l2_factory.latest()?)));

                let l2_payload_builder = GwynethPayloadBuilder::default();
                let output = l2_payload_builder.try_build(
                    l2_builder_args(
                        blockchain_db, 
                        l2_spec.clone(), 
                        l2_genesis_block.seal_slow(), 
                        builder_attrs
                    )
                )
                .map_err(|e| ef_tests::Error::Assertion(e.to_string()))?;

                if let BuildOutcome::Better { payload, cached_reads } = output {
                    Ok(())
                } else {
                    Err(Error::Assertion("L2 Payload failed".to_string())) 
                }
            })?;
        
        Ok(())
    }
}

fn l2_chain_spec() -> ChainSpec {
    ChainSpecBuilder::default()
        .chain(gwyneth::exex::CHAIN_ID.into())
        .genesis(
            serde_json::from_str(include_str!(
                "../../../crates/ethereum/node/tests/assets/genesis.json"
            ))
            .unwrap(),
        )
        .cancun_activated()
        .build()
}


fn l2_builder_args<DB: Debug + Send + Sync, Client>(
    client: Client, 
    chain_spec: Arc<ChainSpec>,
    parent_block: SealedBlock,
    attr: GwynethPayloadBuilderAttributes<DB>
) -> BuildArguments<NoopTransactionPool, Client, GwynethPayloadBuilderAttributes<DB>, EthBuiltPayload>
{
    let config = PayloadConfig::new(
        Arc::new(parent_block),
        Bytes::default(),
        attr,
        chain_spec
    );
    BuildArguments {
        client,
        pool: NoopTransactionPool::default(),
        cached_reads: CachedReads::default(),
        config,
        cancel: Cancelled::default(),
        best_payload: None,
    }
}

fn execute_l1_case_and_commit<DB: Database>(provider_factory: &ProviderFactory<DB>, case: &BlockchainTest) -> Result<Option<SealedBlock>, Error> {
     // Create a new test database and initialize a provider for the test case.
     let provider = provider_factory.provider_rw().unwrap();

    // Insert initial test state into the provider.
    provider.insert_historical_block(
        SealedBlock::new(
            case.genesis_block_header.clone().into(),
            BlockBody::default(),
        )
        .try_seal_with_senders()
        .unwrap(),
    )?;
    case.pre.write_to_db(provider.tx_ref())?;

    // Initialize receipts static file with genesis
    {
        let mut receipts_writer = provider
            .static_file_provider()
            .latest_writer(StaticFileSegment::Receipts)
            .unwrap();
        receipts_writer.increment_block(0).unwrap();
        receipts_writer.commit_without_sync_all().unwrap();
    }

    // Decode and insert blocks, creating a chain of blocks for the test case.
    let last_block = case.blocks.iter().try_fold(None, |_, block| {
        let decoded = SealedBlock::decode(&mut block.rlp.as_ref())?;
        provider.insert_historical_block(
            decoded.clone().try_seal_with_senders().unwrap(),
        )?;
        Ok::<Option<SealedBlock>, Error>(Some(decoded))
    })?;
    provider
        .static_file_provider()
        .latest_writer(StaticFileSegment::Headers)
        .unwrap()
        .commit_without_sync_all()
        .unwrap();

    // Execute the execution stage using the EVM processor factory for the test case
    // network.
    let _ = ExecutionStage::new_with_executor(
        reth_evm_ethereum::execute::EthExecutorProvider::ethereum(Arc::new(
            case.network.clone().into(),
        )),
    )
    .execute(
        &provider,
        ExecInput { target: last_block.as_ref().map(|b| b.number), checkpoint: None },
    );
    provider.commit()?;

    Ok(last_block)
}