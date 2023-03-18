mod deps;
mod grpc;
mod settings;
mod storage;
mod types;
use common::errors::{die, Result};
use deps::dependency_injector;
use grpc::GrpcProvider;
use log::info;
use runtime_injector::Svc;

#[tokio::main]
async fn main() {
    match run().await {
        Ok(()) => info!("shutting down"),
        Err(err) => die(err),
    }
}

async fn run() -> Result<()> {
    env_logger::init();
    let injector = dependency_injector()?;
    let grpc_provider: Svc<dyn GrpcProvider> = injector.get().unwrap();
    grpc_provider.start().await?;
    Ok(())
}
