#![feature(async_closure)]
#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![recursion_limit = "512"]
#![feature(test)]
// #![feature(trivial_bounds)]
// #![deny(clippy::todo)]

mod deps;
mod grpc;
mod ledger;
mod malice;
mod p2p;
mod settings;
mod storage;
mod types;
mod util;
mod verifier;

use deps::dependency_injector;
use grpc::IGrpcHandler;
use log::info;
use p2p::swarm::ISwarm;
use runtime_injector::Svc;
use tokio::try_join;
use util::{die, Res};
use verifier::IVerifier;

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
    let grpc_handler: Svc<dyn IGrpcHandler> = injector.get()?;
    let kad: Svc<dyn ISwarm> = injector.get()?;
    let verifier: Svc<dyn IVerifier> = injector.get()?;
    let malice: Svc<Box<dyn malice::IMalice>> = injector.get()?;
    try_join!(
        grpc_handler.start(),
        kad.start(),
        verifier.start(),
        malice.start()
    )
    .map(|_| ())
}
