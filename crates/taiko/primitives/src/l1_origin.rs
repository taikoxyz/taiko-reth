use reth_codecs::{main_codec, Compact};
use reth_db::{
    table::{Decode, Encode},
    tables, DatabaseError, TableType, TableViewer,
};
use reth_db_api::table::{Compress, Decompress};
use reth_primitives::{BlockNumber, B256};
use serde::{Deserialize, Serialize};
use std::fmt;
reth_db_api::impl_compression_for_compact!(L1Origin);

/// The key for the latest l1 origin
#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
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

tables! {
    /// Stores the l1 origin of the block
    table L1Origins<Key = BlockNumber, Value = L1Origin>;

    /// Stores the latest l1 origin
    table HeadL1Origin<Key = HeadL1OriginKey, Value = BlockNumber>;
}

/// L1Origin represents a L1Origin of a L2 block.
#[main_codec]
#[derive(Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct L1Origin {
    /// The block number of the l2 block
    pub block_id: BlockNumber,
    /// The hash of the l2 block
    pub l2_block_hash: B256,
    /// The height of the l1 block
    pub l1_block_height: BlockNumber,
    /// The hash of the l1 block
    pub l1_block_hash: B256,
}
