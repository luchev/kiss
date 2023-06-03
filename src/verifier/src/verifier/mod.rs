use std::borrow::BorrowMut;
use std::ops::DerefMut;
use std::time::{Instant, SystemTime};

use crate::grpc::keeper_client::IKeeperGateway;
use crate::ledger::ImmuLedger;
use crate::types::Contract;
use crate::{grpc::keeper_client::KeeperGateway, ledger::ILedger};
use async_trait::async_trait;
use common::Res;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
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
        let keeper_gateway: Svc<Mutex<KeeperGateway>> =
            injector.get().expect("keeper gateway not provided");
        let ledger: Svc<Mutex<ImmuLedger>> = injector.get().expect("ledger not provided");

        Ok(Verifier {
            keeper_gateway,
            ledger,
        })
    }
}

#[async_trait]
pub trait IVerifier: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Verifier {
    keeper_gateway: Svc<Mutex<KeeperGateway>>,
    ledger: Svc<Mutex<ImmuLedger>>,
}

#[async_trait]
impl IVerifier for Verifier {
    async fn start(&self) -> Res<()> {
        loop {
            info!("fetching contracts");
            let mut ledger = self.ledger.lock().await;
            let contracts = ledger.get_contracts().await.unwrap();
            let time_before_start = Instant::now();
            for contract in contracts {
                let mut keeper_gateway = self.keeper_gateway.lock().await;
                let hash_in_swarm = keeper_gateway.verify(contract.file_uuid.clone()).await;
                if let Err(e) = hash_in_swarm {
                    info!("file {} not found in swarm, {}", contract.file_uuid, e);
                    continue;
                }
                if hash_in_swarm.unwrap() != contract.file_hash {
                    info!("hashes are not equal for file {}", contract.file_uuid);
                }
            }
            tokio::time::sleep_until(tokio::time::Instant::from_std(Instant::from(
                time_before_start + Duration::minutes(1),
            )))
            .await;
        }
    }
}
