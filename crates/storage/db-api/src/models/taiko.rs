//! Taiko specific models
use crate::{
    table::{Decode, Encode},
    DatabaseError,
};
use serde::{Deserialize, Serialize};

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
