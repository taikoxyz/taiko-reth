use reth_node_api::{
    validate_version_specific_fields, AttributesValidationError, EngineApiMessageVersion,
    EngineTypes, PayloadOrAttributes,
};
use reth_payload_builder::{
    TaikoBuiltPayload, TaikoExecutionPayloadEnvelope, TaikoPayloadAttributes,
    TaikoPayloadBuilderAttributes,
};
use reth_primitives::ChainSpec;
use reth_rpc_types::{engine::ExecutionPayloadEnvelopeV2, ExecutionPayloadV1};
use serde::{Deserialize, Serialize};

/// The types used in the default mainnet ethereum beacon consensus engine.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct TaikoEngineTypes;

impl EngineTypes for TaikoEngineTypes {
    type PayloadAttributes = TaikoPayloadAttributes;
    type PayloadBuilderAttributes = TaikoPayloadBuilderAttributes;
    type BuiltPayload = TaikoBuiltPayload;
    type ExecutionPayloadV1 = ExecutionPayloadV1;
    type ExecutionPayloadV2 = ExecutionPayloadEnvelopeV2;
    type ExecutionPayloadV3 = TaikoExecutionPayloadEnvelope;

    fn validate_version_specific_fields(
        chain_spec: &ChainSpec,
        version: EngineApiMessageVersion,
        payload_or_attrs: PayloadOrAttributes<'_, TaikoPayloadAttributes>,
    ) -> Result<(), AttributesValidationError> {
        validate_version_specific_fields(chain_spec, version, payload_or_attrs)
    }
}
