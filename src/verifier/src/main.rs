#![feature(async_closure)]

mod deps;
mod grpc;
mod ledger;
mod settings;
mod types;
mod verifier;

use common::{die, Res};
use deps::dependency_injector;
use log::info;
use runtime_injector::{Svc};
use tokio::try_join;
use verifier::IVerifier;
use grpc::verifier_grpc::{verifier_grpc_server::VerifierGrpc, StoreRequest};

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
    let verifier: Svc<dyn IVerifier> = injector.get().unwrap();

    // let _ = try_join!(grpc_handler.start(), verifier.start()).map(|_| ());

    let x = async move || -> Res<()> {
        let grpc_handler: Svc<dyn IGrpcHandler> = injector.get().unwrap();
        let inner = grpc_handler.inner().unwrap();
        for i in 0..100000 {
            let _ = inner.store(tonic::Request::new(StoreRequest {
                name: format!("key{}", i),
                content: format!("value{}", i).into_bytes(),
                ttl: 6000,
            })).await;
        }
        Ok(())
    };
    try_join!(x()).map(|_| ())
}
