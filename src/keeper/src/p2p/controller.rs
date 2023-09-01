use crate::types::{Bytes, OneReceiver, Responder};
use async_trait::async_trait;
use common::Res;
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
    async fn set(&self, key: String, value: Bytes) -> Res<()>;
    async fn get(&self, key: String) -> Res<Bytes>;
}

pub struct SwarmController {
    swarm_api: Svc<Mutex<mpsc::Sender<SwarmInstruction>>>,
}

#[async_trait]
impl ISwarmController for SwarmController {
    async fn set(&self, key: String, value: Bytes) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<()>>>();

        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::Put {
                key,
                value,
                resp: sender,
            })
            .await
            .unwrap();
        let receiving_channel = receiver.await.unwrap();
        let result = receiving_channel.await.unwrap();
        info!("put result: {:?}", result);

        // let (sender, receiver) = oneshot::channel::<QueryId>();
        result
    }

    async fn get(&self, key: String) -> Res<Bytes> {
        let (sender, receiver) = oneshot::channel::<OneReceiver<Res<Bytes>>>();
        self.swarm_api
            .lock()
            .await
            .send(SwarmInstruction::Get { key, resp: sender })
            .await
            .unwrap();
        let receiving_channel = receiver.await.unwrap();
        let result = receiving_channel.await.unwrap();
        info!("get result: {:?}", result);
        result
    }
}

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
