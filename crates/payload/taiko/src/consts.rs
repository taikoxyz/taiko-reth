use reth_primitives::{hex::FromHex, keccak256, Address};
use revm::primitives::FixedBytes;

pub fn golden_touch() -> Address {
    Address::from_hex("0x0000777735367b36bC9B61C50022d9D0700dB4Ec").unwrap()
}

pub static TAIKO_L2_ADDRESS_SUFFIX: &'static str = "10001";

pub fn anchor_selector() -> FixedBytes<4> {
    let hash = keccak256(b"anchor(bytes32,bytes32,uint64,uint32)");
    hash.get(0..4).unwrap().try_into().unwrap()
}

pub static ANCHOR_GAS_LIMIT: u64 = 250_000;
