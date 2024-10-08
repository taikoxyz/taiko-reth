//! Reth RPC interface definitions
//!
//! Provides all RPC interfaces.
//!
//! ## Feature Flags
//!
//! - `client`: Enables JSON-RPC client support.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod admin;
mod anvil;
mod bundle;
mod debug;
mod engine;
mod eth;
mod eth_filter;
mod eth_pubsub;
mod ganache;
mod hardhat;
mod mev;
mod net;
mod optimism;
mod otterscan;
mod reth;
mod rpc;
mod trace;
mod txpool;
mod validation;
mod web3;

/// re-export of all server traits
pub use servers::*;

/// Aggregates all server traits.
pub mod servers {
    pub use crate::{
        admin::AdminApiServer,
        bundle::{EthBundleApiServer, EthCallBundleApiServer},
        debug::DebugApiServer,
        engine::{EngineApiServer, EngineEthApiServer},
        eth::EthApiServer,
        eth_filter::EthFilterApiServer,
        eth_pubsub::EthPubSubApiServer,
        mev::MevApiServer,
        net::NetApiServer,
        otterscan::OtterscanServer,
        reth::RethApiServer,
        rpc::RpcApiServer,
        trace::TraceApiServer,
        txpool::TxPoolApiServer,
        validation::BlockSubmissionValidationApiServer,
        web3::Web3ApiServer,
    };

    #[cfg(feature = "taiko")]
    pub use crate::taiko::{PreBuiltTxList, TaikoApiServer, TaikoAuthApiServer};
}

/// re-export of all client traits
#[cfg(feature = "client")]
pub use clients::*;

/// Aggregates all client traits.
#[cfg(feature = "client")]
pub mod clients {
    pub use crate::{
        admin::AdminApiClient,
        anvil::AnvilApiClient,
        bundle::{EthBundleApiClient, EthCallBundleApiClient},
        debug::DebugApiClient,
        engine::{EngineApiClient, EngineEthApiClient},
        eth::EthApiClient,
        eth_filter::EthFilterApiClient,
        ganache::GanacheApiClient,
        hardhat::HardhatApiClient,
        mev::MevApiClient,
        net::NetApiClient,
        otterscan::OtterscanClient,
        rpc::RpcApiServer,
        trace::TraceApiClient,
        txpool::TxPoolApiClient,
        validation::BlockSubmissionValidationApiClient,
        web3::Web3ApiClient,
    };

    #[cfg(feature = "taiko")]
    pub use crate::taiko::TaikoApiClient;
}

#[cfg(feature = "taiko")]
mod taiko;
