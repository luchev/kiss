mod settings;
mod storage;
mod types;

use common::errors::{die, Result};
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
    let mut builder = Injector::builder();
    builder.provide(
        LocalStorage::default
            .singleton()
            .with_interface::<dyn Storage>(),
    );

    let injector = builder.build();
    let storage: Svc<dyn Storage> = injector.get().unwrap();
    storage.put("file1".into(), vec![b'c']).await;
    println!(
        "{}",
        String::from_utf8(storage.get("file1".into()).await.unwrap()).unwrap()
    );

    Ok(())
}
