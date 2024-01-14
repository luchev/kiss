use std::collections::HashSet;

use crate::{p2p::swarm::QueryGetResponse, Res};
use libp2p_identity::PeerId;
use tokio::sync::oneshot;
use uuid::Uuid;

pub type Bytes = Vec<u8>;
pub type Responder<T> = oneshot::Sender<T>;
pub type OneSender<T> = oneshot::Sender<T>;
pub type OneReceiver<T> = oneshot::Receiver<T>;

#[derive(Debug)]
pub enum SwarmInstruction {
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
}

#[derive(Debug)]
pub struct Contract {
    pub contract_uuid: String,
    pub peer_id: PeerId,
    pub file_uuid: String,
    pub file_hash: String,
    pub upload_date: i64,
    pub ttl: i64,
}
