use tokio::sync::oneshot;

pub type Bytes = Vec<u8>;
#[derive(Debug, Default)]
pub struct Contract {
    pub contract_uuid: String,
    pub file_uuid: String,
    pub file_hash: String,
    pub upload_date: i64,
    pub ttl: i64,
}
