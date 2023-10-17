use sha3::{Digest, Sha3_256};

use crate::types::Bytes;

pub fn hash(content: &Bytes) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}
