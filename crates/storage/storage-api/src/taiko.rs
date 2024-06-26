use reth_primitives::BlockNumber;
use reth_rpc_types::engine::L1Origin;
use reth_storage_errors::provider::ProviderResult;

/// The trait for fetch L1 origin related data.
#[auto_impl::auto_impl(&, Arc)]
pub trait L1OriginReader: Send + Sync {
    /// Get the L1 origin for the given block hash.
    fn get_l1_origin(&self, block_hash: BlockNumber) -> ProviderResult<Option<L1Origin>>;
    /// Get the head L1 origin.
    fn get_head_l1_origin(&self) -> ProviderResult<Option<BlockNumber>>;
}

/// The trait for updating L1 origin related data.
#[auto_impl::auto_impl(&, Arc)]
pub trait L1OriginWriter: Send + Sync {
    /// Save the L1 origin for the given block hash.
    fn save_l1_origin(&self, block_hash: BlockNumber, l1_origin: L1Origin) -> ProviderResult<()>;
    /// Save the head L1 origin.
    fn save_head_l1_origin(&self, block_hash: BlockNumber) -> ProviderResult<()>;
}
