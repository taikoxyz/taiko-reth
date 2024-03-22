//! Error type

/// Taiko specific payload building errors.
#[derive(Debug, thiserror::Error)]
pub enum TaikoPayloadBuilderError {
    /// Thrown when a transaction fails to convert to a
    /// [reth_primitives::TransactionSignedEcRecovered].
    #[error("failed to convert deposit transaction to TransactionSignedEcRecovered")]
    TransactionEcRecoverFailed,
    /// Thrown when the L1 block info could not be parsed from the calldata of the
    /// first transaction supplied in the payload attributes.
    #[error("failed to parse L1 block info from L1 info tx calldata")]
    L1BlockInfoParseFailed,
    /// Thrown when a database account could not be loaded.
    #[error("failed to load account {0}")]
    AccountLoadFailed(reth_primitives::Address),
    /// Thrown when force deploy of create2deployer code fails.
    #[error("failed to force create2deployer account code")]
    ForceCreate2DeployerFail,
    /// Thrown when a blob transaction is included in a sequencer's block.
    #[error("blob transaction included in sequencer block")]
    BlobTransactionRejected,
    /// Thrown when a invalid anchor transaction is included in a sequencer's block.
    #[error("invalid anchor transaction included in sequencer block")]
    InvalidAnchorTransaction,
    /// Thrown when a transaction is not able to be marked as anchor.
    #[error("failed to mark anchor")]
    FailedToMarkAnchor,
    /// Thrown when a transaction is not able to be decoded from the payload.
    #[error("failed to decode tx")]
    FailedToDecodeTx,
}
