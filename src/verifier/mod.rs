pub mod por;

use crate::ledger::{ILedger, ImmuLedger};
use crate::p2p::controller::ISwarmController;
use crate::settings::ISettings;
use crate::util::debug::print_now;
use crate::util::{consts, Res};
use crate::util::{Er, ErrorKind};
use async_trait::async_trait;
use base64::Engine as _;
use libp2p::PeerId;
use libp2p_identity::Keypair;
use log::{debug, info, warn};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::str::FromStr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use time::{Duration, OffsetDateTime, Time};
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
        let settings: Svc<dyn ISettings> = injector.get()?;

        let local_key = match settings.swarm().keypair {
            Some(keypair) => Keypair::from_protobuf_encoding(
                &base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(keypair)
                    .map_err(|e| InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<Verifier>(),
                        inner: Box::<Er>::new(ErrorKind::KeypairBase64DecodeError(e).into()),
                    })?,
            )
            .map_err(|e| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<Verifier>(),
                inner: Box::<Er>::new(ErrorKind::KeypairBase64DecodingError(e).into()),
            })?,
            None => {
                return Err(InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<Verifier>(),
                    inner: Box::<Er>::new(
                        ErrorKind::Generic("verifier requires a peer id in the config".to_string())
                            .into(),
                    ),
                })
            }
        };

        let local_peer_id = PeerId::from(local_key.public());
        Ok(Verifier {
            ledger,
            swarm_controller,
            iteration: Mutex::new(1),
            starting_uuid: Mutex::new(0),
            ending_uuid: Mutex::new(u128::MAX / consts::NUM_PEERS),
            peer_id: local_peer_id,
            corrupt: settings.verifier().corrupt,
        })
    }
}

#[async_trait]
pub trait IVerifier: Service {
    async fn start(&self) -> Res<()>; // TALK can't be mut
}

pub struct Verifier {
    ledger: Svc<Mutex<ImmuLedger>>,
    swarm_controller: Svc<dyn ISwarmController>,
    iteration: Mutex<u128>,
    starting_uuid: Mutex<u128>,
    ending_uuid: Mutex<u128>,
    peer_id: PeerId,
    corrupt: bool,
}

#[async_trait]
impl IVerifier for Verifier {
    async fn start(&self) -> Res<()> {
        loop {
            let contracts = {
                let mut ledger = self.ledger.lock().await;
                ledger.get_all_contracts().await?
            };
            let time_before_start = Instant::now();

            let starting_uuid = *self.starting_uuid.lock().await;
            let ending_uuid = *self.ending_uuid.lock().await;
            let mut audits = vec![];
            for contract in contracts {
                let uuid_uint = Uuid::from_str(contract.file_uuid.as_str())
                    .unwrap_or_default()
                    .as_u128();
                if uuid_uint < starting_uuid || uuid_uint > ending_uuid {
                    debug!("skipping: {}", contract.file_uuid);
                    continue;
                }

                let verification_client =
                    VerificationClient::new(VerificationClientConfig::from_contract(&contract));
                let challenge = verification_client.make_challenge_vector();
                let response = self
                    .swarm_controller
                    .request_verification(
                        contract.peer_id,
                        contract.file_uuid.clone(),
                        challenge.clone(),
                    )
                    .await;

                let mut is_success = false;
                match response {
                    Ok(response) => match verification_client.audit(challenge, response) {
                        true => {
                            self.reward_peer(contract.peer_id).await;
                            audits.push((contract.contract_uuid.clone(), true));
                            is_success = true;
                        }
                        false => {
                            // print_now(format!("audit failed for: {}", contract.file_uuid).as_str());
                            self.punish_peer(contract.peer_id).await;
                            audits.push((contract.contract_uuid.clone(), false));
                        }
                    },
                    Err(_) => {
                        // print_now(format!("audit failed for: {}", contract.file_uuid).as_str());
                        self.punish_peer(contract.peer_id).await;
                        audits.push((contract.contract_uuid.clone(), false));
                    }
                }

                let mut previous = self
                    .ledger
                    .lock()
                    .await
                    .get_previous_verified(contract.contract_uuid.clone())
                    .await?;
                previous.sort_by(|a, b| a.verification_time.cmp(&b.verification_time).reverse());

                if let Some(last) = previous.first() {
                    let now = SystemTime::now();
                    let last_time = UNIX_EPOCH + Duration::milliseconds(last.verification_time);
                    let elapsed = now.duration_since(last_time)?;
                    if is_success != last.succeeded {
                        info!(
                            "verifier {} caught cheating after {} ms",
                            contract.peer_id,
                            elapsed.as_millis()
                        );
                    }
                }
            }

            self.next_iteration().await;
            info!(
                "iteration: {}, successfully verified: {}, corrupted: {}",
                *self.iteration.lock().await,
                audits.iter().filter(|(_, succeeded)| *succeeded).count(),
                audits.iter().filter(|(_, succeeded)| !*succeeded).count()
            );

            for (contract_uuid, is_success) in audits {
                let res = if !self.corrupt {
                    self.ledger
                        .lock()
                        .await
                        .create_verified_claim(contract_uuid, self.peer_id, is_success)
                        .await
                } else {
                    self.ledger
                        .lock()
                        .await
                        .create_verified_claim(contract_uuid, self.peer_id, !is_success)
                        .await
                };
                match res {
                    Ok(_) => {}
                    Err(e) => {
                        // warn!("failed to create verified claim: {}", e);
                    }
                }
            }

            tokio::time::sleep_until(tokio::time::Instant::from_std(
                time_before_start + consts::VERIFICATION_CYCLE_TIME,
            ))
            .await;
        }
    }
}

impl Verifier {
    pub async fn punish_peer(&self, peer_id: PeerId) {
        let rep = self
            .ledger
            .lock()
            .await
            .get_reputation(peer_id)
            .await
            .unwrap();
        // time now in seconds:
        let now = OffsetDateTime::now_utc().unix_timestamp();
        info!("{} other peer rep: {}", now, rep);

        let res = {
            self.ledger
                .lock()
                .await
                .decrease_reputation(peer_id, consts::AUDIT_PENALTY)
                .await
        };
        debug!("decreasing reputation after punishment result: {:?}", res)
    }

    pub async fn reward_peer(&self, peer_id: PeerId) {
        let res = {
            self.ledger
                .lock()
                .await
                .increase_reputation(peer_id, consts::AUDIT_REWARD)
                .await
        };
        debug!("increasing reputation after audit result: {:?}", res)
    }

    async fn next_iteration(&self) {
        let mut iteration = self.iteration.lock().await;
        let mut starting_uuid = self.starting_uuid.lock().await;
        let mut ending_uuid = self.ending_uuid.lock().await;
        *iteration += 1;
        if *iteration == consts::NUM_PEERS + 1 {
            *iteration = 0;
            *starting_uuid = 0;
            *ending_uuid = *starting_uuid + u128::MAX / consts::NUM_PEERS;
        } else if *iteration == consts::NUM_PEERS {
            *starting_uuid += u128::MAX / consts::NUM_PEERS;
            *ending_uuid = u128::MAX;
        } else {
            *starting_uuid += u128::MAX / consts::NUM_PEERS;
            *ending_uuid = *starting_uuid + u128::MAX / consts::NUM_PEERS;
        }
    }
}
