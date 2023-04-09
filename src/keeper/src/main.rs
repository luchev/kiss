#![feature(async_closure)]

mod deps;
mod grpc;
mod settings;
mod storage;
mod types;
mod p2p;

use common::{die, Res};
use deps::dependency_injector;
use grpc::IGrpcHandler;
use log::info;
use p2p::{ISwarm, Instruction};
use runtime_injector::Svc;
use tokio::{try_join, sync::{Mutex, mpsc, oneshot}};

#[tokio::main]
async fn main() {
    match run().await {
        Ok(()) => info!("shutting down"),
        Err(err) => die(err),
    }
}

async fn run() -> Res<()> {
    env_logger::init();
    let injector = dependency_injector()?;
    // let receiver: Svc<Mutex<mpsc::Receiver<>>> = injector.get().unwrap();
    let sender: Svc<Mutex<mpsc::Sender<Instruction>>> = injector.get().unwrap();
    let (resp_tx, resp_rx) = oneshot::channel::<()>();

    sender.lock().await.send(Instruction::Put { key: "key1".into(), val: "val1".into(), resp: resp_tx }).await.unwrap();

    let x = async || {
        let res = resp_rx.await;
        println!("res: {:?}", res);
        Ok(())
    };

    let grpc_handler: Svc<dyn IGrpcHandler> = injector.get().unwrap();
    let kad: Svc<dyn ISwarm> = injector.get().unwrap();
    try_join!(grpc_handler.start(), kad.start(), x()).map(|_| ())
}
