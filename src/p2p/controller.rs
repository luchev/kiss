use std::collections::HashSet;

use crate::util::types::{Bytes, OneReceiver};
use crate::util::{types::SwarmInstruction, Res};
use async_trait::async_trait;
use libp2p_identity::PeerId;
use log::info;
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
        let sender: Svc<Mutex<mpsc::Sender<SwarmInstruction>>> = injector.get()?;
        Ok(SwarmController { swarm_api: sender })
    }
}

#[async_trait]
pub trait ISwarmController: Service {
    async fn put(&self, key: String, value: Bytes) -> Res<()>;
    async fn put_to(&self, key: String, value: Bytes, peers: Vec<PeerId>) -> Res<()>;
    async fn get(&self, key: String) -> Res<Bytes>;
    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>>;
    async fn get_closest_peers(&self, key: Uuid) -> Res<Vec<PeerId>>;
    async fn start_providing(&self, key: String) -> Res<()>;
}

pub struct SwarmController {
    swarm_api: Svc<Mutex<mpsc::Sender<SwarmInstruction>>>,
}

#[async_trait]
impl ISwarmController for SwarmController {
    async fn put(&self, key: String, value: Bytes) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::PutLocal {
                key,
                value,
                resp: sender,
            })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("put result: {:?}", result);

        // let (sender, receiver) = oneshot::channel::<QueryId>();
        result
    }

    async fn put_to(&self, key: String, value: Bytes, peer_ids: Vec<PeerId>) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::PutRemote {
                key,
                value,
                resp: sender,
                remotes: peer_ids,
            })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("put to result: {:?}", result);

        // let (sender, receiver) = oneshot::channel::<QueryId>();
        result
    }

    async fn start_providing(&self, key: String) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::StartProviding { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("start providing: {:?}", result);
        result
    }

    async fn get(&self, key: String) -> Res<Bytes> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<Bytes>>>();
        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::Get { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("get result: {:?}", result);
        result
    }

    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<HashSet<PeerId>>>>();
        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::GetProviders { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("get providers result: {:?}", result);
        result
    }

    async fn get_closest_peers(&self, key: Uuid) -> Res<Vec<PeerId>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<Vec<PeerId>>>>();
        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::GetClosestPeers { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("get closest peers result: {:?}", result);
        result
    }
}
