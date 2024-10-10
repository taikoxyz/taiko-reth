use jsonrpsee::{
    core::client::ClientT,
    http_client::{transport::HttpBackend, HttpClient},
};
use reth_ethereum_engine_primitives::ExecutionPayloadEnvelopeV3;
use reth_node_api::EngineTypes;
use reth_node_core::args::RpcServerArgs;
use reth_payload_builder::PayloadId;
use reth_primitives::B256;
use reth_provider::CanonStateNotificationStream;
use reth_rpc_api::EngineApiClient;
use reth_rpc_layer::AuthClientService;
use reth_rpc_types::{
    engine::{ForkchoiceState, PayloadStatusEnum},
    ExecutionPayloadV3,
};
use std::{marker::PhantomData, net::Ipv4Addr};
use reth_rpc_builder::constants;

/// Helper for engine api operations
pub struct EngineApiContext<E> {
    pub canonical_stream: CanonStateNotificationStream,
    pub engine_api_client: HttpClient<AuthClientService<HttpBackend>>,
    pub _marker: PhantomData<E>,
}

impl<E: EngineTypes> EngineApiContext<E> {
    /// Retrieves a v3 payload from the engine api
    pub async fn get_payload_v3(
        &self,
        payload_id: PayloadId,
    ) -> eyre::Result<E::ExecutionPayloadV3> {
        Ok(EngineApiClient::<E>::get_payload_v3(&self.engine_api_client, payload_id).await?)
    }

    /// Retrieves a v3 payload from the engine api as serde value
    pub async fn get_payload_v3_value(
        &self,
        payload_id: PayloadId,
    ) -> eyre::Result<serde_json::Value> {
        Ok(self.engine_api_client.request("engine_getPayloadV3", (payload_id,)).await?)
    }

    /// Submits a payload to the engine api
    pub async fn submit_payload(
        &self,
        payload: E::BuiltPayload,
        parent_beacon_block_root: B256,
        expected_status: PayloadStatusEnum,
        versioned_hashes: Vec<B256>,
    ) -> eyre::Result<B256>
    where
        E::ExecutionPayloadV3: From<E::BuiltPayload> + PayloadEnvelopeExt,
    {
        // setup payload for submission
        let envelope_v3: <E as EngineTypes>::ExecutionPayloadV3 = payload.into();

        // submit payload to engine api
        let submission = EngineApiClient::<E>::new_payload_v3(
            &self.engine_api_client,
            envelope_v3.execution_payload(),
            versioned_hashes,
            parent_beacon_block_root,
        )
        .await?;

        assert_eq!(submission.status, expected_status);

        Ok(submission.latest_valid_hash.unwrap_or_default())
    }

    /// Sends forkchoice update to the engine api
    pub async fn update_forkchoice(&self, current_head: B256, new_head: B256) -> eyre::Result<()> {
        EngineApiClient::<E>::fork_choice_updated_v2(
            &self.engine_api_client,
            ForkchoiceState {
                head_block_hash: new_head,
                safe_block_hash: current_head,
                finalized_block_hash: current_head,
            },
            None,
        )
        .await?;
        Ok(())
    }

    /// Sends forkchoice update to the engine api with a zero finalized hash
    pub async fn update_optimistic_forkchoice(&self, hash: B256) -> eyre::Result<()> {
        EngineApiClient::<E>::fork_choice_updated_v2(
            &self.engine_api_client,
            ForkchoiceState {
                head_block_hash: hash,
                safe_block_hash: B256::ZERO,
                finalized_block_hash: B256::ZERO,
            },
            None,
        )
        .await?;

        Ok(())
    }
}

/// The execution payload envelope type.
pub trait PayloadEnvelopeExt: Send + Sync + std::fmt::Debug {
    /// Returns the execution payload V3 from the payload
    fn execution_payload(&self) -> ExecutionPayloadV3;
}

impl PayloadEnvelopeExt for ExecutionPayloadEnvelopeV3 {
    fn execution_payload(&self) -> ExecutionPayloadV3 {
        self.execution_payload.clone()
    }
}
pub trait RpcServerArgsExEx {
    fn with_static_l2_rpc_ip_and_port(self, chain_id: u64) -> Self;
}

impl RpcServerArgsExEx for RpcServerArgs {
    fn with_static_l2_rpc_ip_and_port(mut self, chain_id: u64) -> Self {
        self.http = true;
        // On the instance the program is running, we wanna have 10111 exposed as the (exex) L2's
        // RPC port.
        self.http_addr = Ipv4Addr::new(0, 0, 0, 0).into();
        self.http_port = 10110u16;
        self.ws_port = 10111u16;
        self.ipcpath = format!("{}-{}", constants::DEFAULT_IPC_ENDPOINT, chain_id);
        self
    }
}
