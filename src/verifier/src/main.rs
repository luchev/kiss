#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

mod deps;
mod grpc;
mod ledger;
mod settings;
mod types;
mod verifier;

use common::{die, Res};
use deps::dependency_injector;
use log::info;
use runtime_injector::Svc;
use tokio::try_join;
use verifier::IVerifier;

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
    try_join!(grpc_handler.start(), verifier.start()).map(|_| ())
}
