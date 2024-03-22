use reth_primitives::{hex::FromHex, keccak256, Address, Buf};
use revm::primitives::FixedBytes;

pub const GOLDE_TOUCH_ACCOUNT: Address =
    Address::from_hex("0x0000777735367b36bC9B61C50022d9D0700dB4Ec");

pub const TAIKO_L2_ADDRESS_SUFFIX: &'static str = "10001";

pub const ANCHOR_SELECTOR: FixedBytes<4> =
    keccak256(b"anchor(bytes32,bytes32,uint64,uint32)").take(4);

pub const ANCHOR_GAS_LIMIT: u64 = 250_000;
