pub mod por;

use crate::ledger::{ILedger, ImmuLedger};
use crate::p2p::controller::ISwarmController;
use crate::util::hasher::hash;
use crate::util::Res;
use async_trait::async_trait;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::str::FromStr;
use std::time::Instant;
use time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

interface! {
    dyn IVerifier = [
        Verifier,
    ]
}

pub struct VerifierProvider;
impl ServiceFactory<()> for VerifierProvider {
    type Result = Verifier;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let ledger: Svc<Mutex<ImmuLedger>> = injector.get()?;
        let swarm_controller = injector.get::<Svc<dyn ISwarmController>>()?;
        Ok(Verifier {
            ledger,
            swarm_controller,
        })
    }
}

#[async_trait]
pub trait IVerifier: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Verifier {
    ledger: Svc<Mutex<ImmuLedger>>,
    swarm_controller: Svc<dyn ISwarmController>,
}

#[async_trait]
impl IVerifier for Verifier {
    async fn start(&self) -> Res<()> {
        let mut iteration = 1;
        let num_peers = 3_u128;
        let mut starting_uuid = 0_u128;
        let mut ending_uuid = starting_uuid + u128::MAX / num_peers;
        // TODO enable verifier
        // return Ok(());
        loop {
            // info!("fetching contracts");
            let contracts = {
                let mut ledger = self.ledger.lock().await;
                ledger.get_all_contracts().await?
            };
            let time_before_start = Instant::now();
            for contract in contracts {
                let (file_uuid, file_hash, peer_id) =
                    (contract.file_uuid, contract.file_hash, contract.peer_id);

                let uuid_uint = Uuid::from_str(file_uuid.as_str())
                    .unwrap_or_default()
                    .as_u128();
                if uuid_uint < starting_uuid || uuid_uint > ending_uuid {
                    info!("skipping: {}", file_uuid);
                    continue;
                }

                let swarm_result = self.swarm_controller.get(file_uuid.clone()).await;
                match swarm_result {
                    Ok(x) => {
                        info!(
                            "file found at peer: {}, expected: {}",
                            peer_id, x.origin_peer_id
                        );
                        let swarm_hash = hash(x.file.to_owned().as_slice());
                        if swarm_hash == file_hash {
                            info!("file verified: {}", file_uuid);
                        } else {
                            info!("file not verified: {}", file_uuid);
                        }
                    }
                    Err(e) => {
                        info!("file not found: {}", file_uuid);
                    }
                }
            }
            iteration += 1;
            if iteration == num_peers + 1 {
                iteration = 0;
                starting_uuid = 0;
                ending_uuid = starting_uuid + u128::MAX / num_peers;
            } else if iteration == num_peers {
                starting_uuid += u128::MAX / num_peers;
                ending_uuid = u128::MAX;
            } else {
                starting_uuid += u128::MAX / num_peers;
                ending_uuid = starting_uuid + u128::MAX / num_peers;
            }
            tokio::time::sleep_until(tokio::time::Instant::from_std(
                time_before_start + Duration::minutes(1),
            ))
            .await;
        }
    }
}
