use std::collections::BTreeMap;

use crate::result::ToRpcResult;
use alloy_primitives::Address;
use async_trait::async_trait;
use jsonrpsee::core::RpcResult;
use reth_evm::{ConfigureEvm, ConfigureEvmEnv};
use reth_primitives::{Header, IntoRecoveredTransaction};
use reth_provider::{
    BlockNumReader, BlockReader, ChainSpecProvider, EvmEnvProvider, StateProvider,
    StateProviderFactory,
};
use reth_revm::database::StateProviderDatabase;
use reth_rpc_api::{PreBuiltTxList, TaikoApiServer, TaikoAuthApiServer};
use reth_rpc_types::{txpool::TxpoolContent, Transaction};
use reth_rpc_types_compat::transaction::from_recovered;
use reth_tasks::TaskSpawner;
use reth_transaction_pool::{AllPoolTransactions, PoolTransaction, TransactionPool};
use revm::{db::CacheDB, Database, DatabaseCommit, Evm};
use revm_primitives::{BlockEnv, CfgEnvWithHandlerCfg, EnvWithHandlerCfg, U256};
use taiko_reth_evm::TaikoEvmConfig;
use taiko_reth_primitives::L1Origin;
use taiko_reth_provider::L1OriginReader;

/// Taiko API.
#[derive(Debug)]
pub struct TaikoApi<Provider, Pool> {
    provider: Provider,
    evm_config: TaikoEvmConfig,
    pool: Pool,
    max_gas_limit: u64,
}

struct TaikoApiInner<Provider, Pool> {
    provider: Provider,
    pool: Pool,
    task_spawner: Box<dyn TaskSpawner>,
}

impl<Provider, Pool> TaikoApi<Provider, Pool> {
    /// Creates a new instance of `Taiko`.
    pub fn new(provider: Provider, pool: Pool, max_gas_limit: u64) -> Self {
        Self { provider, evm_config: TaikoEvmConfig::default(), pool, max_gas_limit }
    }
}

impl<Provider, Pool> TaikoApi<Provider, Pool>
where
    Provider: EvmEnvProvider + BlockNumReader,
{
    fn evm<DB: Database>(&self, db: DB) -> RpcResult<Evm<'_, (), DB>> {
        let mut cfg = CfgEnvWithHandlerCfg::new(Default::default(), Default::default());
        let mut block_env = BlockEnv::default();
        let last_block = self.provider.last_block_number().to_rpc_result()?;
        self.provider
            .fill_env_at(&mut cfg, &mut block_env, last_block.into(), self.evm_config)
            .to_rpc_result()?;
        let env = EnvWithHandlerCfg::new_with_cfg_env(cfg, block_env, Default::default());

        Ok(self.evm_config.evm_with_env(db, env))
    }

    fn calc_gas_limit(&self, parent_gas_limit: u64, desired_limit: u64) -> u64 {
        let delta = parent_gas_limit / 1024 - 1;
        let mut limit = parent_gas_limit;
        let desired_limit = std::cmp::max(desired_limit, 5000);
        if limit < desired_limit {
            limit = parent_gas_limit + delta;
            if limit > desired_limit {
                limit = desired_limit;
            }
            return limit;
        }
        if limit > desired_limit {
            limit = parent_gas_limit - delta;
            if limit < desired_limit {
                limit = desired_limit;
            }
        }
        limit
    }
}

#[async_trait]
impl<Provider, Pool> TaikoApiServer for TaikoApi<Provider, Pool>
where
    Provider: L1OriginReader + 'static,
    Pool: TransactionPool + 'static,
{
    /// HeadL1Origin returns the latest L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn head_l1_origin(&self) -> RpcResult<Option<u64>> {
        self.provider.get_head_l1_origin().to_rpc_result()
    }

    /// L1OriginByID returns the L2 block's corresponding L1 origin.
    // #[cfg(feature = "taiko")]
    async fn l1_origin_by_id(&self, block_id: u64) -> RpcResult<L1Origin> {
        self.provider.get_l1_origin(block_id).to_rpc_result()
    }
}

#[async_trait]
impl<Provider, Pool> TaikoAuthApiServer for TaikoApi<Provider, Pool>
where
    Provider: StateProviderFactory + ChainSpecProvider + EvmEnvProvider + BlockNumReader + 'static,
    Pool: TransactionPool + 'static,
{
    /// TxPoolContent retrieves the transaction pool content with the given upper limits.
    async fn tx_pool_content(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Vec<Address>,
        max_transactions_lists: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>> {
        self.tx_pool_content_with_min_tip(
            beneficiary,
            base_fee,
            block_max_gas_limit,
            max_bytes_per_tx_list,
            local_accounts,
            max_transactions_lists,
            0,
        )
        .await
    }

    /// TxPoolContent retrieves the transaction pool content with the given upper limits.
    async fn tx_pool_content_with_min_tip(
        &self,
        beneficiary: Address,
        base_fee: u64,
        block_max_gas_limit: u64,
        max_bytes_per_tx_list: u64,
        local_accounts: Vec<Address>,
        max_transactions_lists: u64,
        min_tip: u64,
    ) -> RpcResult<Vec<PreBuiltTxList>> {
        let mut best_txs = self.pool.best_transactions();
        best_txs.skip_blobs();
        let (locals, remotes): (Vec<_>, Vec<_>) = best_txs
            .filter(|tx| {
                tx.effective_tip_per_gas(base_fee).map_or(false, |tip| tip >= min_tip as u128)
            })
            .partition(|tx| local_accounts.contains(&tx.sender()));
        let mut db =
            CacheDB::new(StateProviderDatabase::new(self.provider.latest().to_rpc_result()?));
        let mut evm = self.evm(db)?;
        let chain_spec = self.provider.chain_spec();
        loop {
            TaikoEvmConfig::fill_tx_env(evm.tx_mut(), transaction, *sender);

            // set the treasury address
            evm.tx_mut().taiko.treasury = chain_spec.treasury();
            evm.transact();

            db.commit(changes);
        }

        todo!()
    }
}
