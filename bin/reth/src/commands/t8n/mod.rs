#![allow(missing_docs)]
//! Main `t8n` command
//!
//! Runs an EVM state transition using revm.

mod mem_db;
mod provider;
mod utils;

use alloy_rlp::Rlp;
use mem_db::*;
use proptest::collection::vec;
use provider::*;
use reth_beacon_consensus::BeaconConsensus;
use reth_revm::{
    eth_dao_fork::{DAO_HARDFORK_BENEFICIARY, DAO_HARDKFORK_ACCOUNTS},
    primitives::{calc_excess_blob_gas, AnalysisKind, ResultAndState},
    revm::{
        primitives::{AccountInfo, BlobExcessGasAndPrice, Bytecode},
        Evm,
    },
    state_change::apply_beacon_root_contract_call,
    DatabaseCommit,
};
use tracing::{info, warn};
use utils::*;

use reth_primitives::{
    basefee::calculate_next_block_base_fee,
    constants::eip4844::MAX_DATA_GAS_PER_BLOCK,
    eip4844::calculate_excess_blob_gas,
    hex, keccak256,
    revm::{config::revm_spec, env::fill_tx_env},
    BaseFeeParams, Bytes, ChainSpec, ForkSpec, Hardfork, Head, Receipt, TxType, U256,
};

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
    fn apply(
        &self,
        prestate: Prestate,
        chain: &ChainSpec,
    ) -> eyre::Result<(MemDb, ExecutionResult, Bytes)> {
        let Prestate { alloc, env, txs } = prestate;
        // set pre state with an in-memory state provider
        let mut db = MemDb::default();
        for (address, account) in alloc {
            let mut info = AccountInfo {
                balance: account.balance,
                nonce: account.nonce,
                ..Default::default()
            };
            if let Some(code) = account.code {
                info.code_hash = keccak256(code.as_ref());
                info.code = Some(Bytecode::new_raw(code));
            }
            db.insert_account_info(address, info);
            for (slot, value) in account.storage {
                db.insert_account_storage(&address, slot, value);
            }
        }
        if chain.fork(Hardfork::Dao).transitions_at_block(env.current_number) {
            let drained_balance: u128 = db.drain_balances(DAO_HARDKFORK_ACCOUNTS).into_iter().sum();
            db.increment_balance(&DAO_HARDFORK_BENEFICIARY, drained_balance);
        }

        let spec_id = revm_spec(
            chain,
            Head::new(
                env.current_number,
                Default::default(),
                env.current_difficulty.unwrap_or_default(),
                U256::MAX,
                env.current_timestamp,
            ),
        );
        let mut evm = Evm::builder()
            .with_db(db)
            .with_spec_id(spec_id)
            .modify_block_env(|blk_env| {
                // set the EVM block environment
                blk_env.number = env.current_number.try_into().unwrap();
                blk_env.coinbase = env.current_coinbase;
                blk_env.timestamp = env.current_timestamp.try_into().unwrap();
                blk_env.difficulty = env.current_difficulty.unwrap_or_default();
                blk_env.prevrandao = env.current_random.map(Into::into);
                blk_env.basefee = env.current_base_fee.unwrap_or_default();
                blk_env.gas_limit = env.current_gas_limit.try_into().unwrap();
                if env.current_excess_blob_gas.is_some() {
                    blk_env.blob_excess_gas_and_price =
                        env.current_excess_blob_gas.map(BlobExcessGasAndPrice::new);
                } else if env.parent_excess_blob_gas.is_some() && env.parent_blob_gas_used.is_some()
                {
                    blk_env.blob_excess_gas_and_price =
                        Some(BlobExcessGasAndPrice::new(calculate_excess_blob_gas(
                            env.parent_excess_blob_gas.unwrap(),
                            env.parent_blob_gas_used.unwrap(),
                        )));
                }
            })
            .modify_cfg_env(|cfg_env| {
                // set the EVM configuration
                cfg_env.chain_id = chain.chain.id();
                cfg_env.perf_analyse_created_bytecodes = AnalysisKind::Analyse;
            })
            .build();

        if env.parent_beacon_block_root.is_some() {
            apply_beacon_root_contract_call(
                &chain,
                env.current_timestamp,
                env.current_number,
                env.parent_beacon_block_root,
                &mut evm,
            )?;
        }

        let mut rejected_txs = vec![];
        let mut included_txs = vec![];
        let mut receipts = vec![];
        let mut blob_gas_used = 0;
        let mut gas_used = 0;

        for (idx, tx) in txs.into_iter().enumerate() {
            if tx.tx_type() == TxType::Eip4844 && evm.block().blob_excess_gas_and_price.is_none() {
                let error = "blob tx used but field env.ExcessBlobGas missing";
                warn!(name: "rejected tx", index = idx, hash = ?tx.hash, error = error);
                rejected_txs.push(RejectedTx { index: idx, error: error.to_string() });
                continue;
            }

            let tx = match tx.try_into_ecrecovered() {
                Ok(tx) => tx,
                Err(_) => {
                    let error = "failed to recover transaction";
                    warn!(name: "rejected tx", index = idx, error = error);
                    rejected_txs.push(RejectedTx { index: idx, error: error.to_string() });
                    continue;
                }
            };
            fill_tx_env(evm.tx_mut(), tx.as_ref(), tx.signer());
            let tx_blob_gas = tx.blob_gas_used().unwrap_or_default();
            let (used, max) = (blob_gas_used + tx_blob_gas, MAX_DATA_GAS_PER_BLOCK);
            if used > max {
                let error = format!("blob gas ({}) would exceed maximum allowance {}", used, max);
                warn!(name: "rejected tx", index = idx, error = error);
                rejected_txs.push(RejectedTx { index: idx, error });
                continue;
            }
            let ResultAndState { result, state } = match evm.transact() {
                Ok(result) => result,
                Err(err) => {
                    info!(
                        name: "rejected tx",
                        index = idx,
                        hash = ?tx.hash(),
                        from = ?tx.signer(),
                        error = ?err
                    );
                    rejected_txs.push(RejectedTx { index: idx, error: err.to_string() });
                    continue;
                }
            };
            blob_gas_used += tx_blob_gas;
            gas_used += result.gas_used();

            evm.db_mut().commit(state);

            // Push transaction changeset and calculate header bloom filter for receipt.
            #[cfg(not(feature = "optimism"))]
            receipts.push(Receipt {
                tx_type: tx.tx_type(),
                // Success flag was added in `EIP-658: Embedding transaction status code in
                // receipts`.
                success: result.is_success(),
                cumulative_gas_used: gas_used,
                // convert to reth log
                logs: result.into_logs().into_iter().map(Into::into).collect(),
            });

            included_txs.push(tx);
        }
        // reward
        if self.reward >= 0 {
            let block_reward = self.reward as u64;
            let mut miner_reward = block_reward;
            let per_ommer = block_reward / 32;
            for ommer in env.ommers {
                miner_reward += per_ommer;
                let reward = ((8 - ommer.delta) * block_reward) / 8;
                evm.db_mut().increment_balance(&ommer.address, reward as u128);
            }
            evm.db_mut().increment_balance(&env.current_coinbase, miner_reward as u128);
        }
        // withdrawals
        for withdrawal in env.withdrawals {
            let amount = withdrawal.amount_wei();
            evm.db_mut().increment_balance(&withdrawal.address, amount);
        }
        // take db
        let db = std::mem::take(evm.db_mut());
        let exec_result = ExecutionResult {
            state_root: todo!(),
            tx_root: todo!(),
            receipt_root: todo!(),
            logs_hash: todo!(),
            bloom: todo!(),
            receipts: todo!(),
            rejected: todo!(),
            difficulty: todo!(),
            gas_used: todo!(),
            base_fee: todo!(),
            withdrawals_root: todo!(),
            current_excess_blob_gas: todo!(),
            current_blob_gas_used: todo!(),
        };
        let body = alloy_rlp::encode(included_txs);
        Ok((db, exec_result, Bytes::from(body)))
    }

    fn output(
        &self,
        alloc: PrestateAlloc,
        result: ExecutionResult,
        body: Bytes,
    ) -> eyre::Result<()> {
        todo!()
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
        let mut prestate = self.parse_prestate()?;
        let mut chain: ChainSpec = self.fork.clone().into();
        chain.chain = self.chain_id.into();

        apply_london_checks(&mut prestate.env, &chain)?;
        apply_shanghai_checks(&mut prestate.env, &chain)?;
        apply_merge_checks(&mut prestate.env, &chain)?;
        apply_cancun_checks(&mut prestate.env, &chain)?;

        let (db, result, body) = self.apply(prestate, &chain)?;
        let alloc = dump_db(db);
        self.output(alloc, result, body)?;
        Ok(())
    }
}

fn dump_db(db: MemDb) -> PrestateAlloc {
    db.accounts
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                PrestateAccount {
                    balance: v.info.balance,
                    nonce: v.info.nonce,
                    storage: v.storage,
                    code: v.info.code.map(|v| v.bytecode),
                },
            )
        })
        .collect()
}

fn apply_london_checks(env: &mut PrestateEnv, chain: &ChainSpec) -> eyre::Result<()> {
    if !chain.fork(Hardfork::London).active_at_block(env.current_number) {
        return Ok(());
    }
    if env.current_base_fee.is_some() {
        return Ok(());
    }
    if env.parent_base_fee.is_none() || env.current_number == 0 {
        return Err(eyre::eyre!("EIP-1559 config but missing 'currentBaseFee' in env section"));
    }
    env.current_base_fee = Some(
        calculate_next_block_base_fee(
            env.parent_gas_used,
            env.parent_gas_limit,
            env.parent_base_fee.map(|v| v.to()).unwrap_or_default(),
            BaseFeeParams::ethereum(),
        )
        .try_into()
        .unwrap(),
    );
    Ok(())
}

fn apply_shanghai_checks(env: &mut PrestateEnv, chain: &ChainSpec) -> eyre::Result<()> {
    if !(chain.fork(Hardfork::Shanghai).active_at_block(env.current_number) &&
        chain.is_shanghai_active_at_timestamp(env.current_timestamp))
    {
        return Ok(());
    }
    if env.withdrawals.is_empty() {
        return Err(eyre::eyre!("Shanghai config but missing 'withdrawals' in env section"));
    }
    Ok(())
}

fn apply_merge_checks(env: &mut PrestateEnv, chain: &ChainSpec) -> eyre::Result<()> {
    let is_merged = chain.get_final_paris_total_difficulty().is_some() &&
        chain.get_final_paris_total_difficulty().unwrap().is_zero();
    if !is_merged {
        if env.current_difficulty.is_some() {
            return Ok(());
        }
        if env.parent_difficulty.is_none() {
            return Err(eyre::eyre!(
                "currentDifficulty was not provided, and cannot be calculated due to missing parentDifficulty"
            ));
        }
        if env.current_number == 0 {
            return Err(eyre::eyre!("currentDifficulty needs to be provided for block"));
        }
        if env.current_timestamp <= env.parent_timestamp {
            return Err(eyre::eyre!(
                "currentDifficulty cannot be calculated -- currentTime ({}) needs to be after parent time ({})",
                env.current_timestamp,
                env.parent_timestamp
            ));
        }
        // TODO: calculate next block difficulty
        return Ok(());
    }
    if env.current_random.is_none() {
        return Err(eyre::eyre!("post-merge requires currentRandom to be defined in env"));
    }
    if env.current_difficulty.is_some() && !env.current_difficulty.unwrap().is_zero() {
        return Err(eyre::eyre!("post-merge difficulty must be zero (or omitted) in env"));
    }
    Ok(())
}

fn apply_cancun_checks(env: &mut PrestateEnv, chain: &ChainSpec) -> eyre::Result<()> {
    if !(chain.fork(Hardfork::Cancun).active_at_block(env.current_number) &&
        chain.is_cancun_active_at_timestamp(env.current_timestamp))
    {
        env.parent_beacon_block_root = None;
        return Ok(());
    }
    if env.parent_beacon_block_root.is_none() {
        return Err(eyre::eyre!("post-cancun env requires parentBeaconBlockRoot to be set"));
    }
    Ok(())
}
