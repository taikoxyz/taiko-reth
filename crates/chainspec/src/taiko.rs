use core::str::FromStr;
use std::collections::BTreeMap;

use crate::ChainSpec;
use alloy_chains::Chain;
use alloy_genesis::{ChainConfig, Genesis, GenesisAccount};
use once_cell::sync::Lazy;
use revm_primitives::{Address, Bytes, FixedBytes, B256, U256};
use serde::{Deserialize, Serialize};

impl ChainSpec {
    /// Returns the treasury address for the chain.
    #[inline]
    pub fn treasury(&self) -> Address {
        const SUFFIX: &str = "10001";
        let prefix = self.chain().id().to_string();
        Address::from_str(&format!(
            "{prefix}{}{SUFFIX}",
            "0".repeat(Address::len_bytes() * 2 - prefix.len() - SUFFIX.len())
        ))
        .unwrap()
    }
}

// Taiko Chain Configuration, sets the chain_id to the internal devnet L2A by default.
static TAIKO_CHAIN_CONFIG: Lazy<ChainConfig> = Lazy::new(|| ChainConfig {
    chain_id: TaikoNamedChain::TaikoInternalL2A as u64,
    homestead_block: Some(0),
    dao_fork_block: None,
    dao_fork_support: false,
    eip150_block: Some(0),
    eip150_hash: None,
    eip155_block: Some(0),
    eip158_block: Some(0),
    byzantium_block: Some(0),
    constantinople_block: Some(0),
    petersburg_block: Some(0),
    istanbul_block: Some(0),
    muir_glacier_block: Some(0),
    berlin_block: Some(0),
    london_block: Some(0),
    arrow_glacier_block: Some(0),
    gray_glacier_block: Some(0),
    merge_netsplit_block: None,
    shanghai_time: Some(0),
    cancun_time: Some(0),
    terminal_total_difficulty: Some(U256::ZERO),
    terminal_total_difficulty_passed: true,
    ethash: None,
    clique: None,
    extra_fields: Default::default(),
    prague_time: None,
    parlia: None,
    deposit_contract_address: None,
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, strum::IntoStaticStr)] // Into<&'static str>, AsRef<str>, fmt::Display and serde::Serialize
#[derive(strum::VariantNames)] // NamedChain::VARIANTS
#[derive(strum::VariantArray)] // NamedChain::VARIANTS
#[derive(strum::EnumString)] // FromStr, TryFrom<&str>
#[derive(strum::EnumIter)] // NamedChain::iter
#[derive(strum::EnumCount)] // NamedChain::COUNT
#[derive(num_enum::TryFromPrimitive)] // TryFrom<u64>
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[strum(serialize_all = "kebab-case")]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[repr(u64)]
pub enum TaikoNamedChain {
    #[cfg_attr(feature = "serde", serde(alias = "mainnet"))]
    Mainnet = 167000,
    #[cfg_attr(feature = "serde", serde(alias = "taiko-internal-l2a"))]
    TaikoInternalL2A = 167001,
    #[cfg_attr(feature = "serde", serde(alias = "taiko-internal-l2b"))]
    TaikoInternalL2B = 167002,
    #[cfg_attr(feature = "serde", serde(alias = "snaefellsjokull"))]
    Snaefellsjokull = 167003,
    #[cfg_attr(feature = "serde", serde(alias = "askja"))]
    Askja = 167004,
    #[cfg_attr(feature = "serde", serde(alias = "grimsvotn"))]
    Grimsvotn = 167005,
    #[cfg_attr(feature = "serde", serde(alias = "eldfell"))]
    Eldfell = 167006,
    #[cfg_attr(feature = "serde", serde(alias = "jolnir"))]
    Jolnir = 167007,
    #[cfg_attr(feature = "serde", serde(alias = "katla"))]
    Katla = 167008,
    #[cfg_attr(feature = "serde", serde(alias = "hekla"))]
    Hekla = 167009,
}

impl From<TaikoNamedChain> for Chain {
    fn from(val: TaikoNamedChain) -> Self {
        Self::from_id_unchecked(val as u64)
    }
}

pub(crate) fn get_taiko_genesis(chain: TaikoNamedChain) -> Genesis {
    let alloc_str = match chain {
        TaikoNamedChain::Mainnet => {
            include_str!("../res/genesis/taiko/mainnet.json")
        }
        TaikoNamedChain::TaikoInternalL2A => {
            include_str!("../res/genesis/taiko/internal_l2a.json")
        }
        TaikoNamedChain::TaikoInternalL2B => {
            include_str!("../res/genesis/taiko/internal_l2b.json")
        }
        TaikoNamedChain::Snaefellsjokull => {
            include_str!("../res/genesis/taiko/snaefellsjokull.json")
        }
        TaikoNamedChain::Askja => include_str!("../res/genesis/taiko/askja.json"),
        TaikoNamedChain::Grimsvotn => include_str!("../res/genesis/taiko/grimsvotn.json"),
        TaikoNamedChain::Eldfell => include_str!("../res/genesis/taiko/eldfell.json"),
        TaikoNamedChain::Jolnir => include_str!("../res/genesis/taiko/jolnir.json"),
        TaikoNamedChain::Katla => include_str!("../res/genesis/taiko/katla.json"),
        TaikoNamedChain::Hekla => include_str!("../res/genesis/taiko/hekla.json"),
    };

    let alloc: BTreeMap<Address, TaikoGenesisAccount> =
        serde_json::from_str(alloc_str).expect("Invalid alloc json");
    let mut config = TAIKO_CHAIN_CONFIG.clone();
    config.chain_id = chain as u64;

    Genesis {
        config,
        alloc: alloc.into_iter().map(|(k, v)| (k, v.into())).collect(),
        nonce: 0,
        timestamp: 0,
        extra_data: Bytes::new(),
        gas_limit: 15_000_000,
        difficulty: U256::from(0),
        mix_hash: FixedBytes::ZERO,
        coinbase: Address::ZERO,
        base_fee_per_gas: Some(10_000_000),
        excess_blob_gas: None,
        blob_gas_used: Some(0),
        number: Some(0),
    }
}

/// An account in the state of the genesis block.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaikoGenesisAccount {
    /// The nonce of the account at genesis.
    #[serde(skip_serializing_if = "Option::is_none", with = "alloy_serde::quantity::opt", default)]
    pub nonce: Option<u64>,
    /// The balance of the account at genesis.
    pub balance: U256,
    /// The account's bytecode at genesis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,
    /// The account's storage at genesis.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "alloy_serde::storage::deserialize_storage_map"
    )]
    pub storage: Option<BTreeMap<B256, B256>>,
    /// The account's private key. Should only be used for testing.
    #[serde(rename = "secretKey", default, skip_serializing_if = "Option::is_none")]
    pub private_key: Option<B256>,
}

impl From<TaikoGenesisAccount> for GenesisAccount {
    fn from(account: TaikoGenesisAccount) -> Self {
        Self {
            nonce: account.nonce,
            balance: account.balance,
            code: account.code,
            storage: account.storage,
            private_key: account.private_key,
        }
    }
}
