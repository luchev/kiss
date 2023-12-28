mod por;

use crate::ledger::{ILedger, ImmuLedger};
use crate::util::Res;
use async_trait::async_trait;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::time::Instant;
use time::Duration;
use tokio::sync::Mutex;

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
        Ok(Verifier { ledger })
    }
}

#[async_trait]
pub trait IVerifier: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Verifier {
    ledger: Svc<Mutex<ImmuLedger>>,
}

#[async_trait]
impl IVerifier for Verifier {
    async fn start(&self) -> Res<()> {
        loop {
            info!("fetching contracts");
            let contracts = {
                let mut ledger = self.ledger.lock().await;
                ledger.get_contracts().await?
            };
            let time_before_start = Instant::now();
            for contract in contracts {
                // let mut keeper_gateway = self.keeper_gateway.lock().await;
                // let hash_in_swarm = keeper_gateway.verify(contract.file_uuid.clone()).await;
                let hash_in_swarm: Res<String> = Ok("".to_string()); // TODO-FIX ME
                if let Err(e) = hash_in_swarm {
                    info!("file {} not found in swarm, {}", contract.file_uuid, e);
                    continue;
                }
                if hash_in_swarm? != contract.file_hash {
                    info!("hashes are not equal for file {}", contract.file_uuid);
                }
            }
            tokio::time::sleep_until(tokio::time::Instant::from_std(
                time_before_start + Duration::minutes(1),
            ))
            .await;
        }
    }
}
