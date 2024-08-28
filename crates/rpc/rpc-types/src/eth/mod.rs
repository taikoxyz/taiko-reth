//! Ethereum related types

pub(crate) mod error;
pub mod transaction;

/// re-exports
pub mod engine {
    pub use alloy_rpc_types_engine::*;

    use alloy_primitives::B256;
    use alloy_rpc_types::Withdrawal;
    use serde::{Deserialize, Serialize};

    /// This is the input to `engine_newPayloadV2`, which may or may not have a withdrawals field.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ExecutionPayloadInputV2 {
        /// The V1 execution payload
        #[serde(flatten)]
        pub execution_payload: ExecutionPayloadV1,
        /// The payload withdrawals
        #[serde(skip_serializing_if = "Option::is_none")]
        pub withdrawals: Option<Vec<Withdrawal>>,
    }

    /// This is the input to `engine_newPayloadV2`, which may or may not have a withdrawals field.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TaikoExecutionPayloadInputV2 {
        /// The V1 execution payload
        #[serde(flatten)]
        pub execution_payload: ExecutionPayloadInputV2,
        /// Allow passing txHash directly instead of transactions list
        pub tx_hash: B256,
        /// Allow passing `WithdrawalsHash` directly instead of withdrawals
        pub withdrawals_hash: B256,
    }
}
