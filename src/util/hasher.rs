use sha3::{Digest, Sha3_256};

pub fn hash(content: &[u8]) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}
