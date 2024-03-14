use crate::EthPayloadBuilderAttributes;
use alloy_rlp::Error as DecodeError;
use reth_node_api::{BuiltPayload, PayloadBuilderAttributes};
use reth_primitives::{
    revm::config::revm_spec_by_timestamp_after_merge,
    revm_primitives::{BlobExcessGasAndPrice, BlockEnv, CfgEnv, CfgEnvWithHandlerCfg, SpecId},
    Address, BlobTransactionSidecar, ChainSpec, Header, SealedBlock, Withdrawals, B256, U256,
};
use reth_rpc_types::engine::{
    ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3, ExecutionPayloadV1, PayloadAttributes,
    PayloadId,
};
use reth_rpc_types_compat::engine::payload::{
    block_to_payload_v3, convert_block_to_payload_field_v2, try_block_to_payload_v1,
};
use revm_primitives::Bytes;
use serde::{Deserialize, Serialize};
// use std::sync::Arc;

/// BlockMetadata represents a `BlockMetadata` struct defined in protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TaikoBlockMetadata {
    pub beneficiary: Address,
    pub gas_limit: u64,
    pub timestamp: u64,
    pub mix_hash: B256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_list: Option<Vec<Bytes>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highest_block_id: Option<u64>,
    pub extra_data: Vec<u8>,
}

/// L1Origin represents a L1Origin of a L2 block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct L1Origin {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<u64>,
    pub l2_block_hash: B256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l1_block_height: Option<u64>,
    pub l1_block_hash: B256,
}

/// Taiko Payload Attributes
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaikoPayloadAttributes {
    /// The payload attributes
    #[serde(flatten)]
    pub payload_attributes: PayloadAttributes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_fee_per_gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_metadata: Option<TaikoBlockMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub l1_origin: Option<L1Origin>,
}

/// Taiko Payload Builder Attributes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaikoPayloadBuilderAttributes {
    /// Inner ethereum payload builder attributes
    pub payload_attributes: EthPayloadBuilderAttributes,
    /// The base layer fee per gas
    pub base_fee_per_gas: Option<u64>,
    /// Taiko specific block metadata
    pub block_metadata: Option<TaikoBlockMetadata>,
    /// The L1 origin of the L2 block
    pub l1_origin: Option<L1Origin>,
}

impl PayloadBuilderAttributes for TaikoPayloadBuilderAttributes {
    type RpcPayloadAttributes = TaikoPayloadAttributes;
    type Error = DecodeError;

    /// Creates a new payload builder for the given parent block and the attributes.
    ///
    /// Derives the unique [PayloadId] for the given parent and attributes
    fn try_new(parent: B256, attributes: TaikoPayloadAttributes) -> Result<Self, DecodeError> {
        let payload_attributes =
            EthPayloadBuilderAttributes::new(parent, attributes.payload_attributes);

        Ok(Self {
            payload_attributes,
            base_fee_per_gas: attributes.base_fee_per_gas,
            block_metadata: attributes.block_metadata,
            l1_origin: attributes.l1_origin,
        })
    }

    fn payload_id(&self) -> PayloadId {
        self.payload_attributes.id
    }

    fn parent(&self) -> B256 {
        self.payload_attributes.parent
    }

    fn timestamp(&self) -> u64 {
        self.payload_attributes.timestamp
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.payload_attributes.parent_beacon_block_root
    }

    fn suggested_fee_recipient(&self) -> Address {
        self.payload_attributes.suggested_fee_recipient
    }

    fn prev_randao(&self) -> B256 {
        self.payload_attributes.prev_randao
    }

    fn withdrawals(&self) -> &Withdrawals {
        &self.payload_attributes.withdrawals
    }

    fn cfg_and_block_env(
        &self,
        chain_spec: &ChainSpec,
        parent: &Header,
    ) -> (CfgEnvWithHandlerCfg, BlockEnv) {
        // configure evm env based on parent block
        let mut cfg = CfgEnv::default();
        cfg.chain_id = chain_spec.chain().id();

        // ensure we're not missing any timestamp based hardforks
        let spec_id = revm_spec_by_timestamp_after_merge(chain_spec, self.timestamp());

        // if the parent block did not have excess blob gas (i.e. it was pre-cancun), but it is
        // cancun now, we need to set the excess blob gas to the default value
        let blob_excess_gas_and_price = parent
            .next_block_excess_blob_gas()
            .or_else(|| {
                if spec_id.is_enabled_in(SpecId::CANCUN) {
                    // default excess blob gas is zero
                    Some(0)
                } else {
                    None
                }
            })
            .map(BlobExcessGasAndPrice::new);

        // calculate basefee based on parent block's gas usage
        let basefee = U256::from(if let Some(base_fee_per_gas) = &self.base_fee_per_gas {
            *base_fee_per_gas
        } else {
            parent
                .next_block_base_fee(chain_spec.base_fee_params(self.timestamp()))
                .unwrap_or_default()
        });

        let gas_limit = U256::from(if let Some(block_metadata) = &self.block_metadata {
            block_metadata.gas_limit
        } else {
            parent.gas_limit
        });

        let block_env = BlockEnv {
            number: U256::from(parent.number + 1),
            coinbase: self.suggested_fee_recipient(),
            timestamp: U256::from(self.timestamp()),
            difficulty: U256::ZERO,
            prevrandao: Some(self.prev_randao()),
            gas_limit,
            basefee,
            // calculate excess gas based on parent block's blob gas usage
            blob_excess_gas_and_price,
        };

        (CfgEnvWithHandlerCfg::new_with_spec_id(cfg, spec_id), block_env)
    }
}

/// Contains the built payload.
#[derive(Debug, Clone)]
pub struct TaikoBuiltPayload {
    /// Identifier of the payload
    pub(crate) id: PayloadId,
    /// The built block
    pub(crate) block: SealedBlock,
    /// The fees of the block
    pub(crate) fees: U256,
    /// The blobs, proofs, and commitments in the block. If the block is pre-cancun, this will be
    /// empty.
    pub(crate) sidecars: Vec<BlobTransactionSidecar>,
    // /// The rollup's chainspec.
    // pub(crate) chain_spec: Arc<ChainSpec>,
    // /// The payload attributes.
    // pub(crate) attributes: TaikoPayloadBuilderAttributes,
}

// === impl BuiltPayload ===

impl TaikoBuiltPayload {
    /// Initializes the payload with the given initial block.
    pub fn new(
        id: PayloadId,
        block: SealedBlock,
        fees: U256,
        // chain_spec: Arc<ChainSpec>,
        // attributes: TaikoPayloadBuilderAttributes,
    ) -> Self {
        Self { id, block, fees, sidecars: Vec::new() }
    }

    /// Returns the identifier of the payload.
    pub fn id(&self) -> PayloadId {
        self.id
    }

    /// Returns the built block(sealed)
    pub fn block(&self) -> &SealedBlock {
        &self.block
    }

    /// Fees of the block
    pub fn fees(&self) -> U256 {
        self.fees
    }

    /// Adds sidecars to the payload.
    pub fn extend_sidecars(&mut self, sidecars: Vec<BlobTransactionSidecar>) {
        self.sidecars.extend(sidecars)
    }
}

impl BuiltPayload for TaikoBuiltPayload {
    fn block(&self) -> &SealedBlock {
        &self.block
    }

    fn fees(&self) -> U256 {
        self.fees
    }
}

impl<'a> BuiltPayload for &'a TaikoBuiltPayload {
    fn block(&self) -> &SealedBlock {
        (**self).block()
    }

    fn fees(&self) -> U256 {
        (**self).fees()
    }
}

// V1 engine_getPayloadV1 response
impl From<TaikoBuiltPayload> for ExecutionPayloadV1 {
    fn from(value: TaikoBuiltPayload) -> Self {
        try_block_to_payload_v1(value.block)
    }
}

// V2 engine_getPayloadV2 response
impl From<TaikoBuiltPayload> for ExecutionPayloadEnvelopeV2 {
    fn from(value: TaikoBuiltPayload) -> Self {
        let TaikoBuiltPayload { block, fees, .. } = value;

        ExecutionPayloadEnvelopeV2 {
            block_value: fees,
            execution_payload: convert_block_to_payload_field_v2(block),
        }
    }
}

impl From<TaikoBuiltPayload> for ExecutionPayloadEnvelopeV3 {
    fn from(value: TaikoBuiltPayload) -> Self {
        let TaikoBuiltPayload { block, fees, sidecars, .. } = value;

        ExecutionPayloadEnvelopeV3 {
            execution_payload: block_to_payload_v3(block.clone()),
            block_value: fees,
            // From the engine API spec:
            //
            // > Client software **MAY** use any heuristics to decide whether to set
            // `shouldOverrideBuilder` flag or not. If client software does not implement any
            // heuristic this flag **SHOULD** be set to `false`.
            //
            // Spec:
            // <https://github.com/ethereum/execution-apis/blob/fe8e13c288c592ec154ce25c534e26cb7ce0530d/src/engine/cancun.md#specification-2>
            should_override_builder: false,
            blobs_bundle: sidecars.into_iter().map(Into::into).collect::<Vec<_>>().into(),
        }
    }
}
