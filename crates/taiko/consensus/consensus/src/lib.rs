//! Beacon consensus implementation.

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/paradigmxyz/reth/main/assets/reth-docs.png",
    html_favicon_url = "https://avatars0.githubusercontent.com/u/97369466?s=256",
    issue_tracker_base_url = "https://github.com/paradigmxyz/reth/issues/"
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use reth_chainspec::ChainSpec;
use reth_consensus::{Consensus, ConsensusError, PostExecutionInput};
use reth_consensus_common::validation::{
    validate_4844_header_standalone, validate_against_parent_4844,
    validate_against_parent_hash_number, validate_header_base_fee, validate_header_extradata,
    validate_header_gas,
};
use reth_primitives::{
    constants::MAXIMUM_GAS_LIMIT, BlockWithSenders, GotExpected, Header, SealedBlock, SealedHeader,
    EMPTY_OMMER_ROOT_HASH, U256,
};
use std::{sync::Arc, time::SystemTime};

mod validation;
pub use validation::validate_block_post_execution;

mod anchor;
pub use anchor::*;

/// Taiko beacon consensus
///
/// This consensus engine does basic checks as outlined in the execution specs.
#[derive(Debug)]
pub struct TaikoBeaconConsensus {
    /// Configuration
    chain_spec: Arc<ChainSpec>,
}

impl TaikoBeaconConsensus {
    /// Create a new instance of [`EthBeaconConsensus`]
    pub const fn new(chain_spec: Arc<ChainSpec>) -> Self {
        Self { chain_spec }
    }

    /// Checks the gas limit for consistency between parent and self headers.
    ///
    /// The maximum allowable difference between self and parent gas limits is determined by the
    /// parent's gas limit divided by the elasticity multiplier (1024).
    fn validate_against_parent_gas_limit(
        &self,
        header: &SealedHeader,
        _parent: &SealedHeader,
    ) -> Result<(), ConsensusError> {
        if header.gas_limit > MAXIMUM_GAS_LIMIT {
            return Err(ConsensusError::GasLimitInvalidMaximum {
                child_gas_limit: header.gas_limit,
            });
        }

        Ok(())
    }
}

impl Consensus for TaikoBeaconConsensus {
    fn validate_header(&self, header: &SealedHeader) -> Result<(), ConsensusError> {
        // Check if timestamp is in the future. Clock can drift but this can be consensus issue.
        let present_timestamp =
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();

        if header.timestamp > present_timestamp {
            return Err(ConsensusError::TimestampIsInFuture {
                timestamp: header.timestamp,
                present_timestamp,
            });
        }
        validate_header_gas(header)?;
        validate_header_base_fee(header, &self.chain_spec)?;

        if !header.is_zero_difficulty() {
            return Err(ConsensusError::TheMergeDifficultyIsNotZero);
        }

        if header.nonce != 0 {
            return Err(ConsensusError::TheMergeNonceIsNotZero);
        }

        if header.ommers_hash != EMPTY_OMMER_ROOT_HASH {
            return Err(ConsensusError::TheMergeOmmerRootIsNotEmpty);
        }

        // Post-merge, the consensus layer is expected to perform checks such that the block
        // timestamp is a function of the slot. This is different from pre-merge, where blocks
        // are only allowed to be in the future (compared to the system's clock) by a certain
        // threshold.
        //
        // Block validation with respect to the parent should ensure that the block timestamp
        // is greater than its parent timestamp.

        // validate header extradata for all networks post merge
        validate_header_extradata(header)?;

        // EIP-4895: Beacon chain push withdrawals as operations
        if self.chain_spec.is_shanghai_active_at_timestamp(header.timestamp)
            && header.withdrawals_root.is_none()
        {
            return Err(ConsensusError::WithdrawalsRootMissing);
        } else if !self.chain_spec.is_shanghai_active_at_timestamp(header.timestamp)
            && header.withdrawals_root.is_some()
        {
            return Err(ConsensusError::WithdrawalsRootUnexpected);
        }

        // Ensures that EIP-4844 fields are valid once cancun is active.
        if self.chain_spec.is_cancun_active_at_timestamp(header.timestamp) {
            validate_4844_header_standalone(header)?;
        } else if header.blob_gas_used.is_some() {
            return Err(ConsensusError::BlobGasUsedUnexpected);
        } else if header.excess_blob_gas.is_some() {
            return Err(ConsensusError::ExcessBlobGasUnexpected);
        } else if header.parent_beacon_block_root.is_some() {
            return Err(ConsensusError::ParentBeaconBlockRootUnexpected);
        }

        if self.chain_spec.is_prague_active_at_timestamp(header.timestamp) {
            if header.requests_root.is_none() {
                return Err(ConsensusError::RequestsRootMissing);
            }
        } else if header.requests_root.is_some() {
            return Err(ConsensusError::RequestsRootUnexpected);
        }

        Ok(())
    }

    fn validate_header_against_parent(
        &self,
        header: &SealedHeader,
        parent: &SealedHeader,
    ) -> Result<(), ConsensusError> {
        validate_against_parent_hash_number(header, parent)?;

        validate_against_parent_timestamp(header, parent)?;

        // TODO Check difficulty increment between parent and self
        // Ace age did increment it by some formula that we need to follow.
        self.validate_against_parent_gas_limit(header, parent)?;

        // ensure that the blob gas fields for this block
        if self.chain_spec.is_cancun_active_at_timestamp(header.timestamp) {
            validate_against_parent_4844(header, parent)?;
        }

        Ok(())
    }

    fn validate_header_with_total_difficulty(
        &self,
        _header: &Header,
        _total_difficulty: U256,
    ) -> Result<(), ConsensusError> {
        Ok(())
    }

    fn validate_block_pre_execution(&self, block: &SealedBlock) -> Result<(), ConsensusError> {
        // Check ommers hash
        let ommers_hash = reth_primitives::proofs::calculate_ommers_root(&block.ommers);
        if block.header.ommers_hash != ommers_hash {
            return Err(ConsensusError::BodyOmmersHashDiff(
                GotExpected { got: ommers_hash, expected: block.header.ommers_hash }.into(),
            ));
        }

        // Check transaction root
        if let Err(error) = block.ensure_transaction_root_valid() {
            return Err(ConsensusError::BodyTransactionRootDiff(error.into()));
        }

        // EIP-4844: Shard Blob Transactions
        if self.chain_spec.is_cancun_active_at_timestamp(block.timestamp) {
            // Check that the blob gas used in the header matches the sum of the blob gas used by each
            // blob tx
            let header_blob_gas_used =
                block.blob_gas_used.ok_or(ConsensusError::BlobGasUsedMissing)?;
            let total_blob_gas = block.blob_gas_used();
            if total_blob_gas != header_blob_gas_used {
                return Err(ConsensusError::BlobGasUsedDiff(GotExpected {
                    got: header_blob_gas_used,
                    expected: total_blob_gas,
                }));
            }
        }

        // EIP-7685: General purpose execution layer requests
        if self.chain_spec.is_prague_active_at_timestamp(block.timestamp) {
            let requests = block.requests.as_ref().ok_or(ConsensusError::BodyRequestsMissing)?;
            let requests_root = reth_primitives::proofs::calculate_requests_root(&requests.0);
            let header_requests_root =
                block.requests_root.as_ref().ok_or(ConsensusError::RequestsRootMissing)?;
            if requests_root != *header_requests_root {
                return Err(ConsensusError::BodyRequestsRootDiff(
                    GotExpected { got: requests_root, expected: *header_requests_root }.into(),
                ));
            }
        }

        Ok(())
    }

    fn validate_block_post_execution(
        &self,
        block: &BlockWithSenders,
        input: PostExecutionInput<'_>,
    ) -> Result<(), ConsensusError> {
        validate_block_post_execution(block, &self.chain_spec, input.receipts, input.requests)
    }
}

/// Validates the timestamp against the parent to make sure it is in the past.
#[inline]
fn validate_against_parent_timestamp(
    header: &SealedHeader,
    parent: &SealedHeader,
) -> Result<(), ConsensusError> {
    if header.timestamp < parent.timestamp {
        return Err(ConsensusError::TimestampIsInPast {
            parent_timestamp: parent.timestamp,
            timestamp: header.timestamp,
        });
    }
    Ok(())
}

// #[inline]
// fn validate_ommers(header: &SealedHeader) -> Result<(), ConsensusError> {
//     if header.ommers_hash == EMPTY_OMMER_ROOT_HASH  {
//         return Err(ConsensusError::OmmersHashEmpty)
//     }
//     Ok(())

// }
