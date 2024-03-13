#![allow(unreachable_pub)]
use itertools::Itertools;
use rayon::vec;
use reth_beacon_consensus::BeaconConsensus;
use reth_interfaces::consensus::Consensus;
use reth_node_ethereum::EthEvmConfig;
use reth_provider::test_utils::{ExtendedAccount, MockEthProvider};
// TODO: Remove.
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    sync::Arc,
};

use super::try_into_primitive_transaction_and_sign;
use alloy_rlp::Decodable;
use reth_primitives::{
    hex, keccak256, sign_message, Address, Bytes, ChainSpec, TransactionSigned,
    TransactionSignedNoHash, Withdrawal, B256, H256, U256, U64,
};
use reth_revm::{
    primitives::{AccountInfo, Bytecode},
    InMemoryDB,
};
use reth_rpc_types as rpc;

const STDIN_ARG_NAME: &str = "stdin";
const STDOUT_ARG_NAME: &str = "stdout";
const STDERR_ARG_NAME: &str = "stderr";
const RLP_EXT: &str = ".rlp";

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateAccount {
    #[serde(default)]
    pub balance: U256,
    #[serde(default)]
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub storage: HashMap<B256, U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateEnv {
    pub current_coinbase: Address,
    pub current_difficulty: U256,
    pub current_random: U256,
    pub parent_difficulty: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_base_fee: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_gas_used: Option<U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_gas_limit: Option<U64>,
    pub current_number: u64,
    pub current_timestamp: U256,
    pub current_gas_limit: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_timestamp: Option<U256>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub block_hashes: HashMap<u64, B256>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ommers: Vec<Ommer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub withdrawals: Vec<Withdrawal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_base_fee: Option<U256>,
    pub parent_uncle_hash: B256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_blob_gas_used: Option<u64>,
    pub parent_beacon_block_root: B256,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ommer {
    pub delta: u64,
    pub address: Address,
}

pub(crate) type PrestateAlloc = HashMap<Address, PrestateAccount>;

// Input data from stdin
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Input {
    pub alloc: Option<PrestateAlloc>,
    pub env: Option<PrestateEnv>,
    pub txs: Option<Vec<TxWithKey>>,
    pub tx_rlp: Option<String>,
}

impl Input {
    pub fn parse(input_alloc: &str, input_env: &str, input_txs: &str) -> eyre::Result<Prestate> {
        let mut input: Input = Default::default();
        if input_alloc == STDIN_ARG_NAME ||
            input_env == STDIN_ARG_NAME ||
            input_txs == STDIN_ARG_NAME
        {
            input = serde_json::from_reader(std::io::stdin())?;
        }

        if input_alloc != STDIN_ARG_NAME {
            input.alloc = Some(serde_json::from_reader(File::open(input_alloc)?)?);
        }

        if input_env != STDIN_ARG_NAME {
            input.env = Some(serde_json::from_reader(File::open(input_env)?)?);
        }
        let mut txs = vec![];
        if input_txs != STDIN_ARG_NAME {
            if input_txs.ends_with(RLP_EXT) {
                let buf = fs::read(input_txs)?;
                let mut rlp = alloy_rlp::Rlp::new(&buf)?;
                while let Some(tx) = rlp.get_next()? {
                    txs.push(tx);
                }
            } else {
                let tx_with_keys: Vec<TxWithKey> = serde_json::from_reader(File::open(input_txs)?)?;
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
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TxWithKey {
    pub secret_key: Option<SecretKey>,
    #[serde(flatten)]
    pub tx: rpc::Transaction,
    pub protected: bool,
}

pub(crate) struct Prestate {
    pub alloc: PrestateAlloc,
    pub env: PrestateEnv,
    pub txs: Vec<TransactionSigned>,
}

impl Prestate {
    pub(crate) fn apply(self, chain: Arc<ChainSpec>) -> eyre::Result<()> {
        let Prestate { alloc, env, txs } = self;
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
}
