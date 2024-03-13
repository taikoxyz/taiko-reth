#![allow(missing_docs)]
//! Main `t8n` command
//!
//! Runs an EVM state transition using Reth's executor module

mod provider;
mod utils;

use provider::*;
use utils::*;

use reth_primitives::{ChainSpec, ForkSpec};

use clap::Parser;

use std::{path::PathBuf, sync::Arc};

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
    /// Execute `stage` command
    pub async fn execute(&self) -> eyre::Result<()> {
        let prestate = Input::parse(&self.input_alloc, &self.input_env, &self.input_txs)?;

        let mut chain: ChainSpec = self.fork.clone().into();
        chain.chain = self.chain_id.into();
        let chain = Arc::new(chain);

        let _res = prestate.apply(chain)?;
        Ok(())
    }
}
