#![allow(missing_docs)]
//! Main `t8n` command
//!
//! Runs an EVM state transition using Reth's executor module

mod provider;
mod utils;

use provider::*;
use reth_beacon_consensus::BeaconConsensus;
use reth_interfaces::consensus::Consensus;
use reth_node_ethereum::EthEvmConfig;
use reth_provider::test_utils::{ExtendedAccount, MockEthProvider};
use utils::*;

use reth_primitives::{hex, ChainSpec, ForkSpec};

use clap::Parser;

use std::{
    fs::{self, File},
    path::PathBuf,
    sync::Arc,
};

const STDIN_ARG_NAME: &str = "stdin";
const STDOUT_ARG_NAME: &str = "stdout";
const STDERR_ARG_NAME: &str = "stderr";
const RLP_EXT: &str = ".rlp";
/// `reth t8n` command
#[derive(Debug, Parser)]
pub struct Command {
    #[arg(long = "input.alloc")]
    input_alloc: String,
    #[arg(long = "input.env")]
    input_env: String,
    #[arg(long = "input.txs")]
    input_txs: String,
    #[arg(long = "output.basedir")]
    output_basedir: PathBuf,
    #[arg(long = "output.alloc", value_parser = output_source_value_parser)]
    output_alloc: OutputTarget,
    #[arg(long = "output.body", value_parser = output_source_value_parser)]
    output_body: OutputTarget,
    #[arg(long = "output.result", value_parser = output_source_value_parser)]
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
    #[arg(long = "state.fork", value_enum)]
    fork: ForkSpec,
}

impl Command {
    fn apply(&self, prestate: Prestate, chain: Arc<ChainSpec>) -> eyre::Result<()> {
        let Prestate { alloc, env, txs } = prestate;
        // set pre state with an in-memory state provider
        let provider = MockEthProvider::default();
        for (address, account) in alloc {
            let mut reth_account = ExtendedAccount::new(account.nonce, account.balance)
                .extend_storage(account.storage);
            if let Some(code) = account.code {
                reth_account = reth_account.with_bytecode(code);
            }
            provider.add_account(address, reth_account);
        }

        let consensus: Arc<dyn Consensus> = Arc::new(BeaconConsensus::new(Arc::clone(&chain)));

        let factory = reth_revm::EvmProcessorFactory::new(chain, EthEvmConfig::default());

        Ok(())
    }

    fn parse_prestate(&self) -> eyre::Result<Prestate> {
        let mut input: Input = Default::default();
        if self.input_alloc == STDIN_ARG_NAME ||
            self.input_env == STDIN_ARG_NAME ||
            self.input_txs == STDIN_ARG_NAME
        {
            input = serde_json::from_reader(std::io::stdin())?;
        }

        if self.input_alloc != STDIN_ARG_NAME {
            input.alloc = Some(serde_json::from_reader(File::open(&self.input_alloc)?)?);
        }

        if self.input_env != STDIN_ARG_NAME {
            input.env = Some(serde_json::from_reader(File::open(&self.input_env)?)?);
        }
        let mut txs = vec![];
        if self.input_txs != STDIN_ARG_NAME {
            if self.input_txs.ends_with(RLP_EXT) {
                let buf = fs::read(&self.input_txs)?;
                let mut rlp = alloy_rlp::Rlp::new(&buf)?;
                while let Some(tx) = rlp.get_next()? {
                    txs.push(tx);
                }
            } else {
                let tx_with_keys: Vec<TxWithKey> =
                    serde_json::from_reader(File::open(&self.input_txs)?)?;
                for tx in tx_with_keys {
                    let tx = try_into_primitive_transaction_and_sign(tx.tx, &tx.secret_key)?;
                    txs.push(tx);
                }
            }
        } else if input.tx_rlp.is_some() {
            let buf = hex::decode(input.tx_rlp.as_ref().unwrap())?;
            let mut rlp = alloy_rlp::Rlp::new(&buf)?;
            while let Some(tx) = rlp.get_next()? {
                txs.push(tx);
            }
        } else if input.txs.is_some() {
            for tx in input.txs.unwrap() {
                let tx = try_into_primitive_transaction_and_sign(tx.tx, &tx.secret_key)?;
                txs.push(tx);
            }
        }
        Ok(Prestate { alloc: input.alloc.unwrap(), env: input.env.unwrap(), txs })
    }
    /// Execute `stage` command
    pub async fn execute(&self) -> eyre::Result<()> {
        let prestate = self.parse_prestate()?;

        let mut chain: ChainSpec = self.fork.clone().into();
        chain.chain = self.chain_id.into();
        let chain = Arc::new(chain);

        let _res = self.apply(prestate, chain)?;
        Ok(())
    }
}
