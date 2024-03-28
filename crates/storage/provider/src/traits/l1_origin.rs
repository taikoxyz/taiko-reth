use auto_impl::auto_impl;
use reth_interfaces::provider::ProviderResult;
use reth_primitives::L1Origin;

/// Api trait for fetching `L1Origin` related data.
#[auto_impl::auto_impl(&, Arc)]
pub trait L1OriginReader: Send + Sync {
    /// Tries to find in the given blocks L1 origin.
    ///
    /// # Returns
    ///
    /// Returns `None` if the L1 origin is not found.
    fn read_l1_origin(&self, block_id: u64) -> ProviderResult<Option<L1Origin>>;

    /// Tries to find the last L1 origin.
    ///
    /// # Returns
    ///
    /// Returns `None` if the L1 origin is not found.
    fn read_head_l1_origin(&self) -> ProviderResult<Option<u64>>;
}

/// L1 origin Writer
#[auto_impl(&, Arc, Box)]
pub trait L1OriginWriter: Send + Sync {
    /// Insert L1 origin for the given block.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if any operation fails.
    fn insert_l1_origin(&self, block_id: u64, l1_origin: L1Origin) -> ProviderResult<()>;

    /// Inserts the latest L1 origin for the head block.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success, or an error if any operation fails.
    fn insert_head_l1_origin(&self, block_id: u64) -> ProviderResult<()>;
}
