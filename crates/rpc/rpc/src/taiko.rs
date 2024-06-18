use std::collections::BTreeMap;

use alloy_primitives::Address;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use reth_provider::{BlockReader, L1OriginReader};
use reth_rpc_api::TaikoApiServer;
use reth_rpc_types::{txpool::TxpoolContent, Transaction};
use reth_transaction_pool::{AllPoolTransactions, PoolTransaction, TransactionPool};

use crate::result::internal_rpc_err;

/// Taiko API.
#[derive(Debug)]
pub struct TaikoApi<Provider, Pool> {
    provider: Provider,
    pool: Pool,
}

impl<Provider, Pool> TaikoApi<Provider, Pool> {
    /// Creates a new instance of `Taiko`.
    pub const fn new(provider: Provider, pool: Pool) -> Self {
        Self { provider, pool }
    }
}

impl<Provider, Pool> TaikoApi<Provider, Pool>
where
    Provider: BlockReader + L1OriginReader + 'static,
    Pool: TransactionPool + 'static,
{
    fn content(&self) -> TxpoolContent {
        #[inline]
        fn insert<T: PoolTransaction>(
            tx: &T,
            content: &mut BTreeMap<Address, BTreeMap<String, Transaction>>,
        ) {
            content.entry(tx.sender()).or_default().insert(
                tx.nonce().to_string(),
                reth_rpc_types_compat::transaction::from_recovered(tx.to_recovered_transaction()),
            );
        }

        let AllPoolTransactions { pending, queued } = self.pool.all_transactions();

        let mut content = TxpoolContent::default();
        for pending in pending {
            insert(&pending.transaction, &mut content.pending);
        }
        for queued in queued {
            insert(&queued.transaction, &mut content.queued);
        }

        content
    }

    fn get_txs(
        &self,
        locals: &[String],
    ) -> (
        BTreeMap<Address, BTreeMap<String, Transaction>>,
        BTreeMap<Address, BTreeMap<String, Transaction>>,
    ) {
        self.content()
            .pending
            .into_iter()
            .map(|(address, txs)| (address, txs, locals.contains(&address.to_string())))
            .fold(
                (
                    BTreeMap::<Address, BTreeMap<String, Transaction>>::new(),
                    BTreeMap::<Address, BTreeMap<String, Transaction>>::new(),
                ),
                |(mut l, mut r), (address, txs, is_local)| {
                    if is_local {
                        l.insert(address, txs);
                    } else {
                        r.insert(address, txs);
                    }

                    (l, r)
                },
            )
    }

    async fn commit_txs(&self, locals: &[String]) -> RpcResult<Vec<Transaction>> {
        let (_local_txs, _remote_txs) = self.get_txs(&locals);
        Ok(vec![])
    }
}

#[async_trait]
impl<Provider, Pool> TaikoApiServer for TaikoApi<Provider, Pool>
where
    Provider: BlockReader + L1OriginReader + 'static,
    Pool: TransactionPool + 'static,
{
    /// HeadL1Origin returns the latest L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn head_l1_origin(&self) -> RpcResult<Option<u64>> {
        self.provider.read_head_l1_origin().map_err(|_| {
            internal_rpc_err("taiko_headL1Origin failed to read latest l2 block's L1 origin")
        })
    }

    /// L1OriginByID returns the L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn l1_origin_by_id(&self, block_id: u64) -> RpcResult<Option<reth_primitives::L1Origin>> {
        self.provider.read_l1_origin(block_id).map_err(|_| {
            internal_rpc_err("taiko_l1OriginByID failed to read L1 origin by block id")
        })
    }

    /// GetL2ParentHeaders
    // #[cfg(feature = "taiko")]
    async fn get_l2_parent_headers(
        &self,
        block_id: u64,
    ) -> RpcResult<Vec<reth_primitives::Header>> {
        let start = if block_id > 256 { block_id - 255 } else { 0 };
        let mut headers = Vec::with_capacity(256);

        for id in start..=block_id {
            let option = self.provider.header_by_number(id).map_err(|_| {
                internal_rpc_err("taiko_getL2ParentHeaders failed to read header by number")
            })?;
            let Some(header) = option else {
                return Err(internal_rpc_err(
                    "taiko_getL2ParentHeaders failed to find parent header by number",
                ));
            };
            headers.push(header);
        }

        Ok(headers)
    }

    // TODO:(petar) implement this function
    /// TxPoolContent retrieves the transaction pool content with the given upper limits.
    async fn txpool_content(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        locals: Vec<String>,
        max_transactions_lists: u64,
    ) -> RpcResult<Vec<Vec<Transaction>>> {
        let mut tx_lists = Vec::with_capacity(max_transactions_lists as usize);

        for _ in 0..max_transactions_lists {
            let tx_list = self.commit_txs(&locals).await?;

            if tx_list.is_empty() {
                break;
            }

            tx_lists.push(tx_list);
        }

        Ok(tx_lists)
    }
}
