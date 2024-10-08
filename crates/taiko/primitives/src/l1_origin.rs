//! The `L1Origin` module provides the `L1Origin` struct and the `HeadL1Origin` table.
use alloy_rlp::{RlpDecodable, RlpEncodable};
use reth_codecs::{main_codec, Compact};
use reth_db_api::{
    table::{Compress, Decode, Decompress, Encode},
    DatabaseError,
};
use reth_primitives::{B256, U256};
use serde::{Deserialize, Serialize};
reth_db_api::impl_compression_for_compact!(L1Origin);

/// The key for the latest l1 origin
#[derive(Debug, Default, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct HeadL1OriginKey;

impl Encode for HeadL1OriginKey {
    type Encoded = [u8; 1];

    fn encode(self) -> Self::Encoded {
        [0]
    }
}

impl Decode for HeadL1OriginKey {
    fn decode<B: AsRef<[u8]>>(value: B) -> Result<Self, DatabaseError> {
        if value.as_ref() == [0] {
            Ok(Self)
        } else {
            Err(DatabaseError::Decode)
        }
    }
}

/// L1Origin represents a L1Origin of a L2 block.
#[main_codec]
#[derive(Debug, Clone, PartialEq, Eq, RlpDecodable, RlpEncodable)]
#[serde(rename_all = "camelCase")]
pub struct L1Origin {
    /// The block number of the l2 block
    #[serde(rename = "blockID")]
    pub block_id: U256,
    /// The hash of the l2 block
    pub l2_block_hash: B256,
    /// The height of the l1 block
    pub l1_block_height: U256,
    /// The hash of the l1 block
    pub l1_block_hash: B256,
}
