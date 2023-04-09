use crate::types::{Bytes, Responder};
use async_trait::async_trait;
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
        let sender: Svc<Mutex<mpsc::Sender<Instruction>>> = injector.get().unwrap();
        Ok(SwarmController { sender })
    }
}

#[async_trait]
pub trait ISwarmController: Service {
    async fn set(&self);
}

pub struct SwarmController {
    sender: Svc<Mutex<mpsc::Sender<Instruction>>>,
}

#[async_trait]
impl ISwarmController for SwarmController {
    async fn set(&self) {
        let (resp_tx, resp_rx) = oneshot::channel::<()>();

        self.sender
            .lock()
            .await
            .send(Instruction::Put {
                key: "key1".into(),
                val: "val1".into(),
                resp: resp_tx,
            })
            .await
            .unwrap();
        let result = resp_rx.await;
        println!("res: {:?}", result);
    }
}

#[derive(Debug)]
pub enum Instruction {
    Get {
        key: String,
    },
    Put {
        key: String,
        val: Bytes,
        resp: Responder<()>,
    },
}
