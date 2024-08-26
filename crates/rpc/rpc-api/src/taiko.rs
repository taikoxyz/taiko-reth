use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_primitives::Address;
use reth_rpc_types::Transaction;
use serde::{Deserialize, Serialize};
use taiko_reth_primitives::L1Origin;

/// Taiko rpc interface.
#[rpc(server, client, namespace = "taiko")]
pub trait TaikoApi {
    /// HeadL1Origin returns the latest L2 block's corresponding L1 origin.
    #[method(name = "headL1Origin")]
    async fn head_l1_origin(&self) -> RpcResult<L1Origin>;

    /// L1OriginByID returns the L2 block's corresponding L1 origin.
    #[method(name = "l1OriginByID")]
    async fn l1_origin_by_id(&self, block_id: u64) -> RpcResult<L1Origin>;

    /// GetSyncMode returns the node sync mode.
    #[method(name = "getSyncMode")]
    async fn get_sync_mode(&self) -> RpcResult<String> {
        Ok("full".to_string())
    }
}

/// Taiko rpc interface.
#[rpc(server, client, namespace = "taikoAuth")]
pub trait TaikoAuthApi {
    /// Get the transaction pool content.
    #[method(name = "txPoolContent")]
    async fn tx_pool_content(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Option<Vec<Address>>,
        max_transactions_lists: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>>;

    /// Get the transaction pool content with the minimum tip.
    #[method(name = "txPoolContentWithMinTip")]
    async fn tx_pool_content_with_min_tip(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Option<Vec<Address>>,
        max_transactions_lists: u64,
        min_tip: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>>;
}

/// `PreBuiltTxList` is a pre-built transaction list based on the latest chain state,
/// with estimated gas used / bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreBuiltTxList {
    /// The list of transactions.
    pub tx_list: Vec<Transaction>,
    /// The estimated gas used.
    pub estimated_gas_used: u64,
    /// The estimated bytes length.
    pub bytes_length: u64,
}
