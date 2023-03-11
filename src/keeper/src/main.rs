mod settings;
mod storage;
mod types;
mod deps;

use common::errors::{die, Result};
use deps::dependency_injector;
use log::info;
use runtime_injector::{Injector, IntoSingleton, Svc, TypedProvider};
use storage::{local::LocalStorage, Storage};

#[tokio::main]
async fn main() {
    env_logger::init();
    let run = run().await;
    match run {
        Ok(()) => info!("shutting down..."),
        Err(err) => die(err),
    }
}

async fn run() -> Result<()> {
    let injector = dependency_injector()?;
    let storage: Svc<dyn Storage> = injector.get().unwrap();
    storage.put("file1".into(), vec![b'b']).await;
    println!(
        "{}",
        String::from_utf8(storage.get("file1".into()).await.unwrap()).unwrap()
    );

    Ok(())
}
