use std::collections::HashSet;

use crate::{
    p2p::swarm::{QueryGetResponse, VerificationResponse},
    Res,
};
use libp2p_identity::PeerId;
use tokio::sync::oneshot;
use uuid::Uuid;

pub type Bytes = Vec<u8>;
pub type Responder<T> = oneshot::Sender<T>;
pub type OneSender<T> = oneshot::Sender<T>;
pub type OneReceiver<T> = oneshot::Receiver<T>;

#[derive(Debug)]
pub enum CommandToSwarm {
    Get {
        key: String,
        resp: Responder<OneReceiver<Res<QueryGetResponse>>>,
    },
    PutLocal {
        key: String,
        value: Bytes,
        resp: Responder<OneReceiver<Res<()>>>,
    },
    PutRemote {
        key: String,
        value: Bytes,
        remotes: Vec<PeerId>,
        resp: Responder<OneReceiver<Res<()>>>,
    },
    StartProviding {
        key: String,
        resp: Responder<OneReceiver<Res<()>>>,
    },
    GetProviders {
        key: String,
        resp: Responder<OneReceiver<Res<HashSet<PeerId>>>>,
    },
    GetClosestPeers {
        key: Uuid,
        resp: Responder<OneReceiver<Res<Vec<PeerId>>>>,
    },
    RequestVerification {
        peer: PeerId,
        file_uuid: String,
        challenge_vector: Vec<u64>,
        resp: Responder<OneReceiver<Res<VerificationResponse>>>,
    },
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub contract_uuid: String,
    pub peer_id: PeerId,
    pub file_uuid: String,
    pub file_hash: String,
    pub upload_date: i64,
    pub ttl: i64,
    pub secret_n: Vec<u8>,
    pub secret_m: Vec<u8>,
    pub rows: i64,
    pub cols: i64,
}
