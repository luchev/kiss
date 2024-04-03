pub mod por;

use crate::ledger::{ILedger, ImmuLedger};
use crate::p2p::controller::ISwarmController;
use crate::util::hasher::hash;
use crate::util::Res;
use async_trait::async_trait;
use log::{debug, info};
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::str::FromStr;
use std::time::Instant;
use time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

use self::por::{VerificationClient, VerificationClientConfig};

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
        loop {
            let contracts = {
                let mut ledger = self.ledger.lock().await;
                ledger.get_all_contracts().await?
            };
            let time_before_start = Instant::now();
            for contract in contracts {
                let uuid_uint = Uuid::from_str(contract.file_uuid.as_str())
                    .unwrap_or_default()
                    .as_u128();
                if uuid_uint < starting_uuid || uuid_uint > ending_uuid {
                    debug!("skipping: {}", contract.file_uuid);
                    continue;
                }

                let verification_client =
                    VerificationClient::new(VerificationClientConfig::from_contract(
                        contract.secret_n.clone(),
                        contract.secret_m.clone(),
                        contract.rows,
                        contract.cols,
                    ));
                let challenge = verification_client.make_challenge_vector();
                let response = self
                    .swarm_controller
                    .request_verification(
                        contract.peer_id,
                        contract.file_uuid.clone(),
                        challenge.clone(),
                    )
                    .await;
                match response {
                    Ok(response) => match verification_client.audit(challenge, response) {
                        true => info!(
                            "verification successful for file {} at peer {}",
                            contract.file_uuid, contract.peer_id
                        ),
                        false => info!(
                            "verification failed for file {} at peer {}",
                            contract.file_uuid, contract.peer_id
                        ),
                    },
                    Err(_) => info!(
                        "verification failed for file {} at peer {}",
                        contract.file_uuid, contract.peer_id
                    ),
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
