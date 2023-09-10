use crate::Res;
use tokio::sync::oneshot;

pub type Bytes = Vec<u8>;
pub type Responder<T> = oneshot::Sender<T>;
pub type OneSender<T> = oneshot::Sender<T>;
pub type OneReceiver<T> = oneshot::Receiver<T>;

#[derive(Debug)]
pub enum SwarmInstruction {
    Get {
        key: String,
        resp: Responder<OneReceiver<Res<Bytes>>>,
    },
    Put {
        key: String,
        value: Bytes,
        resp: Responder<OneReceiver<Res<()>>>,
    },
}

#[derive(Debug, Default)]
pub struct Contract {
    pub contract_uuid: String,
    pub file_uuid: String,
    pub file_hash: String,
    pub upload_date: i64,
    pub ttl: i64,
}
