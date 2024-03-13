#![allow(unreachable_pub)] // TODO: Remove.
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use reth_primitives::{keccak256, Address, Bytes, Withdrawal, H256, U256, U64};
use reth_revm::{
    primitives::{AccountInfo, Bytecode},
    InMemoryDB,
};
use reth_rpc_types as rpc;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateAccount {
    #[serde(default)]
    pub balance: U256,
    #[serde(default)]
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub storage: HashMap<H256, U256>,
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
    pub block_hashes: HashMap<u64, H256>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ommers: Vec<Ommer>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub withdrawals: Vec<Withdrawal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_base_fee: Option<U256>,
    pub parent_uncle_hash: H256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_blob_gas_used: Option<u64>,
    pub parent_beacon_block_root: H256,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ommer {
    pub delta: u64,
    pub address: Address,
}

pub(crate) type PrestateAlloc = HashMap<Address, PrestateAccount>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Input {
    pub alloc: Option<PrestateAlloc>,
    pub env: Option<PrestateEnv>,
    pub txs: Option<Vec<TxWithKey>>,
    pub tx_rlp: Option<String>,
}

impl Default for Input {
    fn default() -> Self {
        Self { alloc: None, env: None, txs: None, tx_rlp: None }
    }
}

impl From<PrestateAlloc> for Input {
    fn from(value: PrestateAlloc) -> Self {
        Self { alloc: Some(value), ..Default::default() }
    }
}

impl From<PrestateEnv> for Input {
    fn from(value: PrestateEnv) -> Self {
        Self { env: Some(value), ..Default::default() }
    }
}

impl From<Vec<TxWithKey>> for Input {
    fn from(value: Vec<TxWithKey>) -> Self {
        Self { txs: Some(value), ..Default::default() }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TxWithKey {
    pub secret_key: SecretKey,
    #[serde(flatten)]
    pub tx: rpc::Transaction,
    pub protected: bool,
}
