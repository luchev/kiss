mod deps;
mod ledger;
mod settings;

use common::{die, Res};
use deps::dependency_injector;
use ledger::{ILedger, ImmuLedger};
use log::info;
use runtime_injector::Svc;
use std::ops::DerefMut;
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
    let ledger: Svc<Mutex<ImmuLedger>> = injector.get().unwrap();

    let mut inner = ledger.lock().await;
    inner.deref_mut().login().await;
    inner
        .deref_mut()
        .set("k1".to_string(), "val1".to_string())
        .await;
    let val = inner.deref_mut().get("k1".to_string()).await;
    info!("Value of k1 is: {val}");
    Ok(())
}
