//! Helper provider traits to encapsulate all provider traits for simplicity.

#[cfg(feature = "taiko")]
use crate::L1OriginReader;
use crate::{
    AccountReader, BlockReaderIdExt, CanonStateSubscriptions, ChainSpecProvider, ChangeSetReader,
    DatabaseProviderFactory, EvmEnvProvider, StageCheckpointReader, StateProviderFactory,
    StaticFileProviderFactory,
};
use reth_db_api::database::Database;

#[cfg(not(feature = "taiko"))]
/// Helper trait to unify all provider traits for simplicity.
pub trait FullProvider<DB: Database>:
    DatabaseProviderFactory<DB>
    + StaticFileProviderFactory
    + BlockReaderIdExt
    + AccountReader
    + StateProviderFactory
    + EvmEnvProvider
    + ChainSpecProvider
    + ChangeSetReader
    + CanonStateSubscriptions
    + StageCheckpointReader
    + Clone
    + Unpin
    + 'static
{
}

#[cfg(feature = "taiko")]
/// Helper trait to unify all provider traits for simplicity.
pub trait FullProvider<DB: Database>:
    DatabaseProviderFactory<DB>
    + StaticFileProviderFactory
    + BlockReaderIdExt
    + AccountReader
    + StateProviderFactory
    + EvmEnvProvider
    + ChainSpecProvider
    + ChangeSetReader
    + CanonStateSubscriptions
    + StageCheckpointReader
    + L1OriginReader
    + Clone
    + Unpin
    + 'static
{
}

#[cfg(not(feature = "taiko"))]
impl<T, DB: Database> FullProvider<DB> for T where
    T: DatabaseProviderFactory<DB>
        + StaticFileProviderFactory
        + BlockReaderIdExt
        + AccountReader
        + StateProviderFactory
        + EvmEnvProvider
        + ChainSpecProvider
        + ChangeSetReader
        + CanonStateSubscriptions
        + StageCheckpointReader
        + Clone
        + Unpin
        + 'static
{
}

#[cfg(feature = "taiko")]
impl<T, DB: Database> FullProvider<DB> for T where
    T: DatabaseProviderFactory<DB>
        + StaticFileProviderFactory
        + BlockReaderIdExt
        + AccountReader
        + StateProviderFactory
        + EvmEnvProvider
        + ChainSpecProvider
        + ChangeSetReader
        + CanonStateSubscriptions
        + StageCheckpointReader
        + L1OriginReader
        + Clone
        + Unpin
        + 'static
{
}
