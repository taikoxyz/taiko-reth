use crate::EthPayloadBuilderAttributes;
use alloy_rlp::Error as DecodeError;
use reth_node_api::{BuiltPayload, PayloadBuilderAttributes};
use reth_primitives::{
    revm::config::revm_spec_by_timestamp_after_merge,
    revm_primitives::{BlobExcessGasAndPrice, BlockEnv, CfgEnv, CfgEnvWithHandlerCfg, SpecId},
    Address, BlobTransactionSidecar, Bytes, ChainSpec, Header, L1Origin, SealedBlock,
    TaikoBlockMetadata, Withdrawals, B256, U256,
};
use reth_rpc_types::{
    engine::{
        BlobsBundleV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3, ExecutionPayloadV1,
        PayloadAttributes, PayloadId,
    },
    ExecutionPayloadV2, ExecutionPayloadV3,
};
use reth_rpc_types_compat::engine::{
    convert_withdrawal_to_standalone_withdraw,
    payload::{block_to_payload_v3, convert_block_to_payload_field_v2, try_block_to_payload_v1},
};
use serde::{Deserialize, Serialize};
// use std::sync::Arc;

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
    pub l1_origin: L1Origin,
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
    pub l1_origin: L1Origin,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaikoExecutionPayload {
    /// Inner V3 payload
    #[serde(flatten)]
    pub payload_inner: ExecutionPayloadV3,

    /// Allow passing txHash directly instead of transactions list
    pub tx_hash: B256,
    /// Allow passing WithdrawalsHash directly instead of withdrawals
    pub withdrawals_hash: B256,
    /// Whether this is a Taiko L2 block, only used by ExecutableDataToBlock
    pub taiko_block: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaikoExecutionPayloadEnvelope {
    /// Taiko execution payload
    pub execution_payload: TaikoExecutionPayload,
    /// The expected value to be received by the feeRecipient in wei
    pub block_value: U256,
    /// The blobs, commitments, and proofs associated with the executed payload.
    pub blobs_bundle: BlobsBundleV1,
    /// Introduced in V3, this represents a suggestion from the execution layer if the payload
    /// should be used instead of an externally provided one.
    pub should_override_builder: bool,
}

impl From<TaikoBuiltPayload> for TaikoExecutionPayloadEnvelope {
    fn from(value: TaikoBuiltPayload) -> Self {
        let TaikoBuiltPayload { block, fees, sidecars, .. } = value;

        let withdrawals: Vec<reth_rpc_types::withdrawal::Withdrawal> = block
            .withdrawals
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(convert_withdrawal_to_standalone_withdraw)
            .collect();

        TaikoExecutionPayloadEnvelope {
            execution_payload: TaikoExecutionPayload {
                payload_inner: ExecutionPayloadV3 {
                    payload_inner: ExecutionPayloadV2 {
                        payload_inner: ExecutionPayloadV1 {
                            parent_hash: block.parent_hash,
                            fee_recipient: block.beneficiary,
                            state_root: block.state_root,
                            receipts_root: block.receipts_root,
                            logs_bloom: block.logs_bloom,
                            prev_randao: block.mix_hash,
                            block_number: block.number,
                            gas_limit: block.gas_limit,
                            gas_used: block.gas_used,
                            timestamp: block.timestamp,
                            extra_data: block.extra_data.clone(),
                            base_fee_per_gas: U256::from(
                                block.header.base_fee_per_gas.unwrap_or_default(),
                            ),
                            block_hash: block.hash(),
                            transactions: vec![],
                        },
                        withdrawals,
                    },

                    blob_gas_used: block.header.blob_gas_used.unwrap_or_default(),
                    excess_blob_gas: block.header.excess_blob_gas.unwrap_or_default(),
                },
                tx_hash: block.header.transactions_root,
                withdrawals_hash: block.header.withdrawals_root.unwrap_or_default(),
                taiko_block: true,
            },
            block_value: fees,
            blobs_bundle: sidecars.into_iter().map(Into::into).collect::<Vec<_>>().into(),
            should_override_builder: false,
        }
    }
}
