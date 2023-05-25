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
use tokio::sync::Mutex;

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
    let mut gateway: Svc<Mutex<KeeperGateway>> = injector.get().unwrap();
    let _ = gateway.borrow_mut().lock().await.put("k1".to_string(), "value 1".to_string()).await;
    let _ = gateway.borrow_mut().lock().await.get("k1".to_string()).await;

    let ledger: Svc<Mutex<ImmuLedger>> = injector.get().unwrap();
    let mut inner = ledger.lock().await;
    inner
        .deref_mut()
        .set("k1".to_string(), "val1".to_string())
        .await;
    let val = inner.deref_mut().get("k1".to_string()).await;
    info!("Value of k1 is: {val}");
    Ok(())
}
