mod deps;
mod ledger;
mod settings;
mod grpc;
mod types;

use common::{die, Res};
use deps::dependency_injector;
use grpc::keeper_client::{IKeeperGateway, KeeperGateway};
use ledger::{ImmuLedger, ILedger};
use log::info;
use runtime_injector::Svc;
use std::{borrow::BorrowMut, ops::DerefMut};
use tokio::{sync::Mutex, try_join};

use crate::grpc::IGrpcHandler;

pub mod immudb_grpc {
    tonic::include_proto!("immudb.schema");
}

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
    let grpc_handler: Svc<dyn IGrpcHandler> = injector.get().unwrap();
    try_join!(grpc_handler.start()).map(|_| ())

}
