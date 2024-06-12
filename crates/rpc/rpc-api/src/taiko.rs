use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use reth_primitives::Address;
use reth_rpc_types::Transaction;

/// Taiko rpc interface.
#[cfg_attr(not(feature = "client"), rpc(server, namespace = "taiko"))]
#[cfg_attr(feature = "client", rpc(server, client, namespace = "taiko"))]
pub trait TaikoApi {
    /// HeadL1Origin returns the latest L2 block's corresponding L1 origin.
    #[method(name = "headL1Origin")]
    async fn head_l1_origin(&self) -> RpcResult<Option<u64>>;

    /// L1OriginByID returns the L2 block's corresponding L1 origin.
    #[method(name = "l1OriginByID")]
    async fn l1_origin_by_id(&self, block_id: u64) -> RpcResult<Option<reth_primitives::L1Origin>>;

    /// GetL2ParentHeaders
    #[method(name = "getL2ParentHeaders")]
    async fn get_l2_parent_headers(&self, block_id: u64)
        -> RpcResult<Vec<reth_primitives::Header>>;

    /// Returns the details of all transactions currently pending for inclusion in the next
    /// block(s), as well as the ones that are being scheduled for future execution only.
    ///
    /// See [here](https://geth.ethereum.org/docs/rpc/ns-txpool#txpool_content) for more details
    #[method(name = "content")]
    async fn txpool_content(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        locals: Vec<String>,
        max_transactions_lists: u64,
    ) -> RpcResult<Vec<Vec<Transaction>>>;
}
