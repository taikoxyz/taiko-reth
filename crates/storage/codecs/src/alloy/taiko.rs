//! Native Compact codec impl for Taiko types.

use crate::Compact;
use alloy_primitives::{B256, U256};
use alloy_rpc_types_engine::L1Origin as AlloyL1Origin;

use reth_codecs_derive::main_codec;

#[main_codec]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct L1Origin {
    pub block_id: U256,
    pub l2_block_hash: B256,
    pub l1_block_height: U256,
    pub l1_block_hash: B256,
}

impl Compact for AlloyL1Origin {
    fn to_compact<B>(self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let l1_origin = L1Origin {
            block_id: self.block_id,
            l2_block_hash: self.l2_block_hash,
            l1_block_height: self.l1_block_height,
            l1_block_hash: self.l1_block_hash,
        };
        l1_origin.to_compact(buf)
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let (l1_origin, _) = L1Origin::from_compact(buf, len);
        let alloy_l1_origin = Self {
            block_id: l1_origin.block_id,
            l2_block_hash: l1_origin.l2_block_hash,
            l1_block_height: l1_origin.l1_block_height,
            l1_block_hash: l1_origin.l1_block_hash,
        };
        (alloy_l1_origin, buf)
    }
}
