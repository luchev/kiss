use std::collections::HashSet;

use crate::p2p::swarm::{QueryGetResponse, VerificationResponse};
use crate::util::types::{Bytes, OneReceiver};
use crate::util::{types::CommandToSwarm, Res};
use async_trait::async_trait;
use libp2p_identity::PeerId;
use log::{debug, info};
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

interface! {
    dyn ISwarmController = [
        SwarmController,
    ]
}

pub struct SwarmControllerProvider;

impl ServiceFactory<()> for SwarmControllerProvider {
    type Result = SwarmController;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let commands_to_swarm: Svc<Mutex<mpsc::Sender<CommandToSwarm>>> = injector.get()?;
        injector.get()?;
        Ok(SwarmController { commands_to_swarm })
    }
}

#[async_trait]
pub trait ISwarmController: Service {
    async fn put(&self, key: String, value: Bytes) -> Res<()>;
    async fn put_to(&self, key: String, value: Bytes, peers: Vec<PeerId>) -> Res<()>;
    async fn get(&self, key: String) -> Res<QueryGetResponse>;
    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>>;
    async fn get_closest_peers(&self, key: Uuid) -> Res<Vec<PeerId>>;
    async fn start_providing(&self, key: String) -> Res<()>;
    async fn request_verification(
        &self,
        peer: PeerId,
        file_uuid: String,
        secret_vector: Vec<u64>,
    ) -> Res<Vec<u64>>;
}

pub struct SwarmController {
    commands_to_swarm: Svc<Mutex<mpsc::Sender<CommandToSwarm>>>,
}

#[async_trait]
impl ISwarmController for SwarmController {
    async fn put(&self, key: String, value: Bytes) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::PutLocal {
                key,
                value,
                resp: sender,
            })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("put result: {:?}", result);

        result
    }

    async fn put_to(&self, key: String, value: Bytes, peer_ids: Vec<PeerId>) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::PutRemote {
                key,
                value,
                resp: sender,
                remotes: peer_ids,
            })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("put to result: {:?}", result);

        result
    }

    async fn start_providing(&self, key: String) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::StartProviding { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("start providing: {:?}", result);
        result
    }

    async fn get(&self, key: String) -> Res<QueryGetResponse> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<QueryGetResponse>>>();
        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::Get { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("get result: {:?}", result);
        result
    }

    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<HashSet<PeerId>>>>();
        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::GetProviders { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("get providers result: {:?}", result);
        result
    }

    async fn get_closest_peers(&self, key: Uuid) -> Res<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<Vec<PeerId>>>>();
        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::GetClosestPeers { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("get closest peers result: {:?}", result);
        result
    }

    async fn request_verification(
        &self,
        peer: PeerId,
        file_uuid: String,
        challenge_vector: Vec<u64>,
    ) -> Res<Vec<u64>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<VerificationResponse>>>();
        self.commands_to_swarm
            .lock()
            .await
            .send(CommandToSwarm::RequestVerification {
                peer,
                file_uuid,
                challenge_vector,
                resp: sender,
            })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        debug!("request verification result: {:?}", result);
        Ok(result?.response_vector)
    }
}
