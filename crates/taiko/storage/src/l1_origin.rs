//! The module for L1 origin related data.
use reth_db_api::{
    database::Database,
    transaction::{DbTx, DbTxMut},
};
use reth_primitives::BlockNumber;
use reth_provider::{
    providers::BlockchainProvider, DatabaseProvider, DatabaseProviderFactory,
    DatabaseProviderRwFactory,
};
use reth_storage_errors::provider::ProviderResult;
use taiko_reth_primitives::{HeadL1Origin, HeadL1OriginKey, L1Origin, L1Origins};

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
    fn save_l1_origin(&self, block_number: BlockNumber, l1_origin: L1Origin) -> ProviderResult<()>;
    /// Save the head L1 origin.
    fn save_head_l1_origin(&self, block_number: BlockNumber) -> ProviderResult<()>;
}

impl<TX: DbTx> L1OriginReader for DatabaseProvider<TX> {
    fn get_l1_origin(&self, block_number: BlockNumber) -> ProviderResult<Option<L1Origin>> {
        Ok(self.tx_ref().get::<L1Origins>(block_number)?)
    }

    fn get_head_l1_origin(&self) -> ProviderResult<Option<BlockNumber>> {
        Ok(self.tx_ref().get::<HeadL1Origin>(HeadL1OriginKey)?)
    }
}

impl<TX: DbTxMut> L1OriginWriter for DatabaseProvider<TX> {
    fn save_l1_origin(&self, block_number: BlockNumber, l1_origin: L1Origin) -> ProviderResult<()> {
        Ok(self.tx_ref().put::<L1Origins>(block_number, l1_origin)?)
    }

    fn save_head_l1_origin(&self, block_number: BlockNumber) -> ProviderResult<()> {
        Ok(self.tx_ref().put::<HeadL1Origin>(HeadL1OriginKey, block_number)?)
    }
}

impl<DB> L1OriginReader for BlockchainProvider<DB>
where
    DB: Database,
{
    fn get_l1_origin(&self, block_number: BlockNumber) -> ProviderResult<Option<L1Origin>> {
        self.database_provider_ro()?.get_l1_origin(block_number)
    }

    fn get_head_l1_origin(&self) -> ProviderResult<Option<BlockNumber>> {
        self.database_provider_ro()?.get_head_l1_origin()
    }
}

impl<DB> L1OriginWriter for BlockchainProvider<DB>
where
    DB: Database,
{
    fn save_l1_origin(&self, block_number: BlockNumber, l1_origin: L1Origin) -> ProviderResult<()> {
        self.database_provider_rw()?.save_l1_origin(block_number, l1_origin)
    }

    fn save_head_l1_origin(&self, block_number: u64) -> ProviderResult<()> {
        self.database_provider_rw()?.save_head_l1_origin(block_number)
    }
}
