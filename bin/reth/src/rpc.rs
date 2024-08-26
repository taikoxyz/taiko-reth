use std::net::Ipv4Addr;

use alloy_consensus::TxEnvelope;
use alloy_network::eip2718::Decodable2718;
use reth::{
    builder::{rpc::RpcRegistry, FullNodeComponents},
    rpc::{
        api::{
            eth::helpers::{EthApiSpec, EthTransactions, TraceExt},
            DebugApiServer,
        },
        server_types::eth::EthResult,
    },
};
use reth_node_core::args::RpcServerArgs;
use reth_primitives::{Bytes, B256};

use crate::traits::RpcServerArgsExEx;

pub struct RpcTestContext<Node: FullNodeComponents, EthApi> {
    pub inner: RpcRegistry<Node, EthApi>,
}

impl<Node: FullNodeComponents, EthApi> RpcTestContext<Node, EthApi>
where
    EthApi: EthApiSpec + EthTransactions + TraceExt,
{
    /// Injects a raw transaction into the node tx pool via RPC server
    pub async fn inject_tx(&mut self, raw_tx: Bytes) -> EthResult<B256> {
        let eth_api = self.inner.eth_api();
        println!("Dani debug: Why not called? {:?}", raw_tx); // -> Not called most prob. because it is called during actual L2 execution. So the call to advance_block() in our main() will call it.
        eth_api.send_raw_transaction(raw_tx).await
    }

    /// Retrieves a transaction envelope by its hash
    pub async fn envelope_by_hash(&mut self, hash: B256) -> eyre::Result<TxEnvelope> {
        let tx = self.inner.debug_api().raw_transaction(hash).await?.unwrap();
        let tx = tx.to_vec();
        Ok(TxEnvelope::decode_2718(&mut tx.as_ref()).unwrap())
    }
}

impl RpcServerArgsExEx for RpcServerArgs {
    fn with_static_l2_rpc_ip_and_port(mut self) -> Self {
        self.http = true;
        // On the instance the program is running, we wanna have 10111 exposed as the (exex) L2's RPC port.
        self.http_addr = Ipv4Addr::new(0, 0, 0, 0).into();
        self.http_port = 10110u16;
        self.ws_port = 10111u16;
        self
    }
}
