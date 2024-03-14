use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};

use reth_primitives::{
    Address, Bloom, Bytes, Receipt, TransactionSigned, Withdrawal, B256, U256, U64,
};
use reth_revm::revm::primitives::HashMap;
use reth_rpc_types::Transaction;

#[derive(Serialize, PartialEq, Eq)]
pub(crate) struct RejectedTx {
    pub(crate) index: usize,
    pub(crate) error: String,
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutionResult {
    pub(crate) state_root: B256,
    pub(crate) tx_root: B256,
    pub(crate) receipt_root: B256,
    pub(crate) logs_hash: B256,
    pub(crate) bloom: Bloom,
    pub(crate) receipts: Vec<Receipt>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) rejected: Vec<RejectedTx>,
    pub(crate) difficulty: Option<U256>,
    pub(crate) gas_used: U64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) base_fee: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) withdrawals_root: Option<B256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_excess_blob_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) current_blob_gas_used: Option<u64>,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateAccount {
    #[serde(default)]
    pub(crate) balance: U256,
    #[serde(default)]
    pub(crate) nonce: u64,
    pub(crate) storage: HashMap<U256, U256>,
    pub(crate) code: Option<Bytes>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PrestateEnv {
    pub(crate) current_coinbase: Address,
    pub(crate) current_difficulty: Option<U256>,
    pub(crate) current_random: Option<U256>,
    pub(crate) parent_difficulty: Option<U256>,
    pub(crate) parent_base_fee: Option<U256>,
    pub(crate) parent_gas_used: u64,
    pub(crate) parent_gas_limit: u64,
    pub(crate) current_gas_limit: u64,
    pub(crate) current_number: u64,
    pub(crate) current_timestamp: u64,
    pub(crate) parent_timestamp: u64,
    pub(crate) block_hashes: HashMap<U64, B256>,
    pub(crate) ommers: Vec<Ommer>,
    pub(crate) withdrawals: Vec<Withdrawal>,
    pub(crate) current_base_fee: Option<U256>,
    pub(crate) parent_uncle_hash: B256,
    pub(crate) current_excess_blob_gas: Option<u64>,
    pub(crate) parent_excess_blob_gas: Option<u64>,
    pub(crate) parent_blob_gas_used: Option<u64>,
    pub(crate) parent_beacon_block_root: Option<B256>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Ommer {
    pub(crate) delta: u64,
    pub(crate) address: Address,
}

pub(crate) type PrestateAlloc = HashMap<Address, PrestateAccount>;
pub(crate) type Alloc = HashMap<Address, PrestateAccount>;

// Input data from stdin
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Input {
    pub(crate) alloc: Option<PrestateAlloc>,
    pub(crate) env: Option<PrestateEnv>,
    pub(crate) txs: Option<Vec<TxWithKey>>,
    pub(crate) tx_rlp: Option<String>,
}

#[derive(Deserialize)]
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
