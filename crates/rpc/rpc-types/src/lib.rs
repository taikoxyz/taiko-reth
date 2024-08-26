//! Reth RPC type definitions.
//!
//! Provides all relevant types for the various RPC endpoints, grouped by namespace.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#[allow(hidden_glob_reexports)]
mod eth;
mod mev;
mod peer;
mod rpc;

// re-export for convenience
pub use alloy_rpc_types::serde_helpers;

// Ethereum specific rpc types coming from alloy.
pub use alloy_rpc_types::*;

// Ethereum specific serde types coming from alloy.
pub use alloy_serde::*;

pub mod trace {
    //! RPC types for trace endpoints and inspectors.
    pub use alloy_rpc_types_trace::*;
}

// Anvil specific rpc types coming from alloy.
pub use alloy_rpc_types_anvil as anvil;

// re-export beacon
pub use alloy_rpc_types_beacon as beacon;

// re-export admin
pub use alloy_rpc_types_admin as admin;

// re-export txpool
pub use alloy_rpc_types_txpool as txpool;

/// Ethereum specific types for the engine API.
pub mod engine {

    pub use crate::eth::engine::*;

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
}

// Ethereum specific rpc types related to typed transaction requests and the engine API.
pub use eth::{
    engine::{
        ExecutionPayload, ExecutionPayloadV1, ExecutionPayloadV2, ExecutionPayloadV3, PayloadError,
    },
    error::ToRpcError,
    transaction::{self, TransactionRequest, TypedTransactionRequest},
};

pub use mev::*;
pub use peer::*;
pub use rpc::*;
