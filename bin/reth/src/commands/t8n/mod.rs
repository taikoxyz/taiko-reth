#![allow(missing_docs)]
//! Main `t8n` command
//!
//! Runs an EVM state transition using Reth's executor module

mod provider;
mod utils;

use provider::*;
use utils::*;

use crate::{
    args::{
        utils::{chain_help, genesis_value_parser, parse_socket_address, SUPPORTED_CHAINS},
        DatabaseArgs, DebugArgs, DevArgs, NetworkArgs, PayloadBuilderArgs, PruningArgs,
        RpcServerArgs, TxPoolArgs,
    },
    core::cli::runner::CliContext,
    dirs::{DataDirPath, MaybePlatformPath},
};
use reth_beacon_consensus::BeaconConsensus;
use reth_executor::{
    executor::{test_utils::InMemoryStateProvider, Executor},
    revm_wrap::{State, SubState},
};
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::{
    keccak256, Block, ChainSpec, ChainSpecBuilder, ForkCondition, ForkSpec, Header, U256,
};
use reth_provider::{
    test_utils::{ExtendedAccount, MockEthProvider},
    BlockExecutor, ExecutorFactory, PrunableBlockExecutor, StateProvider,
};
use reth_revm::{
    primitives::{AccountInfo, Bytecode},
    InMemoryDB,
};
use reth_rpc_types as rpc;

use clap::Parser;
use serde_with::serde_as;

use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

/// `reth t8n` command
#[derive(Debug, Parser)]
pub struct Command {
    #[arg(long = "input.alloc", value_parser = InputSourceValueParser)]
    input_alloc: InputSource,
    #[arg(long = "input.env", value_parser = InputSourceValueParser)]
    input_env: InputSource,
    #[arg(long = "input.txs", value_parser = InputSourceValueParser)]
    input_txs: InputSource,
    #[arg(long = "output.basedir")]
    output_basedir: PathBuf,
    #[arg(long = "output.alloc", value_parser = OutputTargetValueParser)]
    output_alloc: OutputTarget,
    #[arg(long = "output.body", value_parser = OutputTargetValueParser)]
    output_body: OutputTarget,
    #[arg(long = "output.result", value_parser = OutputTargetValueParser)]
    output_result: OutputTarget,
    #[arg(long)]
    trace: bool,
    #[arg(long = "trace.tracer")]
    tracer: String,
    #[arg(long = "trace.jsonconfig")]
    jsonconfig: String,
    #[arg(long = "trace.memory")]
    memory: bool,
    #[arg(long = "trace.nostack")]
    nostack: bool,
    #[arg(long = "trace.returndata")]
    returndata: bool,
    #[arg(long = "state.reward")]
    reward: i64,
    #[arg(long = "state.chainid")]
    chain_id: u64,
    #[arg(long = "state.fork")]
    chain: ChainSpec,
}

impl Command {
    /// Execute `stage` command
    pub async fn execute(&self) -> eyre::Result<()> {
        let prestate = self.input_alloc.from_json::<Input, PrestateAlloc>()?;
        let env = self.input_env.from_json::<Input, PrestateEnv>()?;
        let txs = self.input_txs.from_json::<Input, Vec<rpc::Transaction>>()?;

        // set pre state with an in-memory state provider
        let provider = MockEthProvider::default();
        for (address, account) in prestate {
            let mut reth_account = ExtendedAccount::new(account.nonce, account.balance)
                .extend_storage(account.storage);
            if let Some(code) = account.code {
                reth_account = reth_account.with_bytecode(code);
            }
            provider.add_account(address, reth_account);
        }

        let mut chain = self.chain.clone();
        chain.chain = self.chain_id.into();
        let chain = Arc::new(chain);

        let consensus: Arc<dyn Consensus> = Arc::new(BeaconConsensus::new(Arc::clone(&chain)));

        let faxctory = reth_revm::EvmProcessorFactory::new(chain, EthEvmConfig::default());

        let executor = factory.with_state(provider);

        let block = Block {
            header: Header {
                beneficiary: env.current_coinbase,
                // TODO: Make RANDAO-aware for post-Shanghai blocks
                difficulty: env.current_difficulty,
                number: env.current_number.as_u64(),
                timestamp: env.current_timestamp.to::<u64>(),
                gas_limit: env.current_gas_limit.to::<u64>(),
                parent_hash: todo!(),
                ommers_hash: env.ommers,
                state_root: todo!(),
                transactions_root: todo!(),
                receipts_root: todo!(),
                withdrawals_root: todo!(),
                logs_bloom: todo!(),
                gas_used: todo!(),
                mix_hash: todo!(),
                nonce: todo!(),
                base_fee_per_gas: todo!(),
                blob_gas_used: todo!(),
                excess_blob_gas: todo!(),
                parent_beacon_block_root: todo!(),
                extra_data: todo!(),
            },
            body: txs.into_iter().map(|x| x.into_transaction()).collect(),
            ..Default::default()
        };
        let mut executor = Executor::new(&spec, &mut provider);
        let result = executor.execute_transactions(&block, U256::ZERO, None);

        // State is committed, so we can try calculating stateroot, txs root etc.
        dbg!(&result);

        Ok(())
    }
}
