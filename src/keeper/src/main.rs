#![feature(async_closure)]

mod deps;
mod grpc;
mod p2p;
mod settings;
mod storage;
mod types;

use common::{die, Res};
use deps::dependency_injector;
use grpc::IGrpcHandler;
use log::info;
use p2p::{controller::ISwarmController, swarm::ISwarm};
use runtime_injector::Svc;
use tokio::try_join;

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
    let sender: Svc<dyn ISwarmController> = injector.get().unwrap();

    let x = async || {
        sender.set().await;
        Ok(())
    };

    let grpc_handler: Svc<dyn IGrpcHandler> = injector.get().unwrap();
    let kad: Svc<dyn ISwarm> = injector.get().unwrap();
    try_join!(grpc_handler.start(), kad.start(), x()).map(|_| ())
}
