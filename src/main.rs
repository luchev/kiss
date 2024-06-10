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

use crate::ledger::{ILedger, ImmuLedger};
use crate::settings::ISettings;
use crate::util::{Er, ErrorKind};
use base64::Engine as _;
use deps::dependency_injector;
use grpc::IGrpcHandler;
use libp2p::PeerId;
use libp2p_identity::Keypair;
use log::{debug, info, warn};
use malice::IMalice;
use p2p::swarm::ISwarm;
use runtime_injector::Svc;
use tokio::{sync::Mutex, try_join};
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
    let malice: Svc<Box<dyn IMalice>> = injector.get()?;
    let settings: Svc<dyn ISettings> = injector.get()?;
    let ledger = injector.get::<Svc<Mutex<ImmuLedger>>>()?;

    if settings.verifier().enabled {
        try_join!(
            grpc_handler.start(),
            kad.start(),
            verifier.start(),
            malice.start(),
            start(ledger, settings),
        )
        .map(|_| ())
    } else {
        try_join!(
            grpc_handler.start(),
            kad.start(),
            malice.start(),
            start(ledger, settings),
        )
        .map(|_| ())
    }
}

async fn start(ledger: Svc<Mutex<ImmuLedger>>, settings: Svc<dyn ISettings>) -> Res<()> {
    let local_key = match settings.swarm().keypair {
        Some(keypair) => Keypair::from_protobuf_encoding(
            &base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(keypair)
                .map_err(|_| {
                    ErrorKind::Generic("eval requires a peer id in the config".to_string())
                })?,
        )
        .map_err(|_| ErrorKind::Generic("eval requires a peer id in the config".to_string()))?,
        None => {
            return Err(
                ErrorKind::Generic("verifier requires a peer id in the config".to_string()).into(),
            )
        }
    };

    let local_peer_id = PeerId::from(local_key.public());

    loop {
        let rep = ledger.lock().await.get_reputation(local_peer_id).await?;
        info!("Current reputation: {:?}", rep);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
