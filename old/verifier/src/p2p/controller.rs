use std::collections::HashSet;

use async_trait::async_trait;
use common::types::{Bytes, OneReceiver};
use common::{types::SwarmInstruction, Res};
use libp2p_identity::PeerId;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use tokio::sync::{mpsc, oneshot, Mutex};

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
    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>>;
}

pub struct SwarmController {
    swarm_api: Svc<Mutex<mpsc::Sender<SwarmInstruction>>>,
}

#[async_trait]
impl ISwarmController for SwarmController {
    async fn get_providers(&self, key: String) -> Res<HashSet<PeerId>> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<HashSet<PeerId>>>>();
        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::GetProviders { key, resp: sender })
            .await?;
        let receiving_channel = receiver.await?;
        let result = receiving_channel.await?;
        info!("get peers result: {:?}", result);
        result
    }
}
