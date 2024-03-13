use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use reth_primitives::{Address, Bytes, TransactionSigned, Withdrawal, B256, U256, U64};
use reth_rpc_types::Transaction;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateAccount {
    #[serde(default)]
    pub(crate) balance: U256,
    #[serde(default)]
    pub(crate) nonce: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub(crate) storage: HashMap<B256, U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateEnv {
    pub(crate) current_coinbase: Address,
    pub(crate) current_difficulty: U256,
    pub(crate) current_random: U256,
    pub(crate) parent_difficulty: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_base_fee: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_gas_used: Option<U64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_gas_limit: Option<U64>,
    pub(crate) current_number: u64,
    pub(crate) current_timestamp: U256,
    pub(crate) current_gas_limit: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_timestamp: Option<U256>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub(crate) block_hashes: HashMap<u64, B256>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) ommers: Vec<Ommer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) withdrawals: Vec<Withdrawal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_base_fee: Option<U256>,
    pub(crate) parent_uncle_hash: B256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_blob_gas_used: Option<u64>,
    pub(crate) parent_beacon_block_root: B256,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ommer {
    pub(crate) delta: u64,
    pub(crate) address: Address,
}

pub(crate) type PrestateAlloc = HashMap<Address, PrestateAccount>;

// Input data from stdin
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Input {
    pub(crate) alloc: Option<PrestateAlloc>,
    pub(crate) env: Option<PrestateEnv>,
    pub(crate) txs: Option<Vec<TxWithKey>>,
    pub(crate) tx_rlp: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TxWithKey {
    pub(crate) secret_key: Option<SecretKey>,
    #[serde(flatten)]
    pub(crate) tx: Transaction,
    pub(crate) protected: bool,
}

pub(crate) struct Prestate {
    pub(crate) alloc: PrestateAlloc,
    pub(crate) env: PrestateEnv,
    pub(crate) txs: Vec<TransactionSigned>,
}
