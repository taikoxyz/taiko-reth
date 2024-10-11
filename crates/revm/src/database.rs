use crate::primitives::alloy_primitives::{BlockNumber, StorageKey, StorageValue};
use core::ops::{Deref, DerefMut};
use reth_primitives::{constants::ETHEREUM_CHAIN_ID, Account, Address, B256, U256};
use reth_storage_api::StateProvider;
use reth_storage_errors::provider::{ProviderError, ProviderResult};
use revm::{
    db::{CacheDB, DatabaseRef},
    primitives::{AccountInfo, Bytecode, ChainAddress},
    Database, SyncDatabase, SyncDatabaseRef,
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SyncStateProviderDatabase<DB>(pub HashMap<u64, StateProviderDatabase<DB>>);

impl<DB> SyncStateProviderDatabase<DB> {
    /// Create new State with generic `StateProvider`.
    pub fn new(chain_id: Option<u64>, db: StateProviderDatabase<DB>) -> Self {
        // assert!(chain_id.is_some());
        let mut map = HashMap::new();
        map.insert(chain_id.unwrap_or(ETHEREUM_CHAIN_ID), db);
        Self(map)
    }

    /// Consume State and return inner `StateProvider`.
    pub fn into_inner(self) -> HashMap<u64, StateProviderDatabase<DB>> {
        self.0
    }

    pub fn add_db(&mut self, chain_id: u64, db: StateProviderDatabase<DB>) {
        self.0.insert(chain_id, db);
    }

    pub fn get_db(&self, chain_id: u64) -> Option<&StateProviderDatabase<DB>> {
        self.0.get(&chain_id)
    }

    pub fn get_db_mut(&mut self, chain_id: u64) -> Option<&mut StateProviderDatabase<DB>> {
        self.0.get_mut(&chain_id)
    }

    pub fn get_default_db(&self) -> Option<&StateProviderDatabase<DB>> {
        self.0.get(&ETHEREUM_CHAIN_ID)
    }

    pub fn get_default_db_mut(&mut self) -> Option<&mut StateProviderDatabase<DB>> {
        self.0.get_mut(&ETHEREUM_CHAIN_ID)
    }
}

impl<DB> Deref for SyncStateProviderDatabase<DB> {
    type Target = HashMap<u64, StateProviderDatabase<DB>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB> DerefMut for SyncStateProviderDatabase<DB> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait SyncEvmStateProvider: Send + Sync {
    /// Get basic account information.
    ///
    /// Returns `None` if the account doesn't exist.
    fn basic_account(&self, address: ChainAddress) -> ProviderResult<Option<Account>>;

    /// Get the hash of the block with the given number. Returns `None` if no block with this number
    /// exists.
    fn block_hash(&self, chain_id: u64, number: BlockNumber) -> ProviderResult<Option<B256>>;

    /// Get account code by its hash
    fn bytecode_by_hash(
        &self,
        chain_id: u64,
        code_hash: B256,
    ) -> ProviderResult<Option<reth_primitives::Bytecode>>;

    /// Get storage of given account.
    fn storage(
        &self,
        account: ChainAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>>;
}

impl<DB: EvmStateProvider> SyncEvmStateProvider for SyncStateProviderDatabase<DB> {
    fn basic_account(&self, address: ChainAddress) -> ProviderResult<Option<Account>> {
        if let Some(db) = self.get(&address.0) {
            db.0.basic_account(address.1)
        } else {
            if address.0 != 1 {
                println!("unknown db: {}", address.0);
            }
            Err(ProviderError::UnsupportedProvider)
        }
    }

    fn block_hash(&self, chain_id: u64, number: BlockNumber) -> ProviderResult<Option<B256>> {
        if let Some(db) = self.get(&chain_id) {
            db.0.block_hash(number)
        } else {
            if chain_id != 1 {
                println!("unknown db: {}", chain_id);
            }
            Err(ProviderError::UnsupportedProvider)
        }
    }

    fn bytecode_by_hash(
        &self,
        chain_id: u64,
        code_hash: B256,
    ) -> ProviderResult<Option<reth_primitives::Bytecode>> {
        if let Some(db) = self.get(&chain_id) {
            db.0.bytecode_by_hash(code_hash)
        } else {
            if chain_id != 1 {
                println!("unknown db: {}", chain_id);
            }
            Err(ProviderError::UnsupportedProvider)
        }
    }

    fn storage(
        &self,
        account: ChainAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        if let Some(db) = self.get(&account.0) {
            db.0.storage(account.1, storage_key)
        } else {
            if account.0 != 1 {
                println!("unknown db: {}", account.0);
            }
            Err(ProviderError::UnsupportedProvider)
        }
    }
}

impl<DB: EvmStateProvider> SyncDatabase for SyncStateProviderDatabase<DB> {
    type Error = ProviderError;

    fn basic(&mut self, address: ChainAddress) -> Result<Option<AccountInfo>, Self::Error> {
        SyncDatabaseRef::basic_ref(self, address)
    }

    fn code_by_hash(&mut self, chain_id: u64, code_hash: B256) -> Result<Bytecode, Self::Error> {
        SyncDatabaseRef::code_by_hash_ref(self, chain_id, code_hash)
    }

    fn storage(&mut self, address: ChainAddress, index: U256) -> Result<U256, Self::Error> {
        SyncDatabaseRef::storage_ref(self, address, index)
    }

    fn block_hash(&mut self, chain_id: u64, number: u64) -> Result<B256, Self::Error> {
        SyncDatabaseRef::block_hash_ref(self, chain_id, number)
    }
}

impl<DB: EvmStateProvider> SyncDatabaseRef for SyncStateProviderDatabase<DB> {
    type Error = <Self as SyncDatabase>::Error;

    fn basic_ref(&self, address: ChainAddress) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.basic_account(address)?.map(Into::into))
    }

    fn code_by_hash_ref(&self, chain_id: u64, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(self.bytecode_by_hash(chain_id, code_hash)?.unwrap_or_default().0)
    }

    fn storage_ref(&self, address: ChainAddress, index: U256) -> Result<U256, Self::Error> {
        Ok(self.storage(address, B256::new(index.to_be_bytes()))?.unwrap_or_default())
    }

    fn block_hash_ref(&self, chain_id: u64, number: u64) -> Result<B256, Self::Error> {
        Ok(self.block_hash(chain_id, number)?.unwrap_or_default())
    }
}

/// A helper trait responsible for providing that necessary state for the EVM execution.
///
/// This servers as the data layer for [Database].
pub trait EvmStateProvider: Send + Sync {
    /// Get basic account information.
    ///
    /// Returns `None` if the account doesn't exist.
    fn basic_account(&self, address: Address) -> ProviderResult<Option<Account>>;

    /// Get the hash of the block with the given number. Returns `None` if no block with this number
    /// exists.
    fn block_hash(&self, number: BlockNumber) -> ProviderResult<Option<B256>>;

    /// Get account code by its hash
    fn bytecode_by_hash(
        &self,
        code_hash: B256,
    ) -> ProviderResult<Option<reth_primitives::Bytecode>>;

    /// Get storage of given account.
    fn storage(
        &self,
        account: Address,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>>;
}

// Blanket implementation of EvmStateProvider for any type that implements StateProvider.
impl<T: reth_storage_api::StateProvider> EvmStateProvider for T {
    fn basic_account(&self, address: Address) -> ProviderResult<Option<Account>> {
        <T as reth_storage_api::AccountReader>::basic_account(self, address)
    }

    fn block_hash(&self, number: BlockNumber) -> ProviderResult<Option<B256>> {
        <T as reth_storage_api::BlockHashReader>::block_hash(self, number)
    }

    fn bytecode_by_hash(
        &self,
        code_hash: B256,
    ) -> ProviderResult<Option<reth_primitives::Bytecode>> {
        <T as reth_storage_api::StateProvider>::bytecode_by_hash(self, code_hash)
    }

    fn storage(
        &self,
        account: Address,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        <T as reth_storage_api::StateProvider>::storage(self, account, storage_key)
    }
}

/// A [Database] and [`DatabaseRef`] implementation that uses [`EvmStateProvider`] as the underlying
/// data source.
#[derive(Debug, Clone)]
pub struct StateProviderDatabase<DB>(pub DB);

impl<DB> StateProviderDatabase<DB> {
    /// Create new State with generic `StateProvider`.
    pub fn new(db: DB) -> Self {
        //println!("Brecht: StateProviderDatabase::new");
        Self(db)
    }

    /// Consume State and return inner `StateProvider`.
    pub fn into_inner(self) -> DB {
        self.0
    }
}

impl<DB> Deref for StateProviderDatabase<DB> {
    type Target = DB;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB> DerefMut for StateProviderDatabase<DB> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<DB: EvmStateProvider> Database for StateProviderDatabase<DB> {
    type Error = ProviderError;

    /// Retrieves basic account information for a given address.
    ///
    /// Returns `Ok` with `Some(AccountInfo)` if the account exists,
    /// `None` if it doesn't, or an error if encountered.
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        //println!("Brecht: read account");
        DatabaseRef::basic_ref(self, address)
    }

    /// Retrieves the bytecode associated with a given code hash.
    ///
    /// Returns `Ok` with the bytecode if found, or the default bytecode otherwise.
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        DatabaseRef::code_by_hash_ref(self, code_hash)
    }

    /// Retrieves the storage value at a specific index for a given address.
    ///
    /// Returns `Ok` with the storage value, or the default value if not found.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        DatabaseRef::storage_ref(self, address, index)
    }

    /// Retrieves the block hash for a given block number.
    ///
    /// Returns `Ok` with the block hash if found, or the default hash otherwise.
    /// Note: It safely casts the `number` to `u64`.
    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        DatabaseRef::block_hash_ref(self, number)
    }
}

impl<DB: EvmStateProvider> DatabaseRef for StateProviderDatabase<DB> {
    type Error = <Self as Database>::Error;

    /// Retrieves basic account information for a given address.
    ///
    /// Returns `Ok` with `Some(AccountInfo)` if the account exists,
    /// `None` if it doesn't, or an error if encountered.
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(self.basic_account(address)?.map(Into::into))
    }

    /// Retrieves the bytecode associated with a given code hash.
    ///
    /// Returns `Ok` with the bytecode if found, or the default bytecode otherwise.
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(self.bytecode_by_hash(code_hash)?.unwrap_or_default().0)
    }

    /// Retrieves the storage value at a specific index for a given address.
    ///
    /// Returns `Ok` with the storage value, or the default value if not found.
    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(self.0.storage(address, B256::new(index.to_be_bytes()))?.unwrap_or_default())
    }

    /// Retrieves the block hash for a given block number.
    ///
    /// Returns `Ok` with the block hash if found, or the default hash otherwise.
    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        // Get the block hash or default hash with an attempt to convert U256 block number to u64
        Ok(self.0.block_hash(number)?.unwrap_or_default())
    }
}

pub struct CachedDBSyncStateProvider<S>(pub CacheDB<SyncStateProviderDatabase<S>>);

impl<S> CachedDBSyncStateProvider<S> {
    pub fn new(db: SyncStateProviderDatabase<S>) -> Self {
        Self(CacheDB::new(db))
    }

    pub fn get_db(&self, chain_id: u64) -> &S {
        &self.0.db.get_db(chain_id).unwrap().0
    }

    pub fn get_db_mut(&mut self, chain_id: u64) -> &mut S {
        self.0.db.get_db_mut(chain_id).unwrap()
    }
}
