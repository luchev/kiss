use std::borrow::BorrowMut;
use std::ops::DerefMut;

use crate::{grpc::keeper_client::KeeperGateway, ledger::ILedger};
use crate::ledger::ImmuLedger;
use async_trait::async_trait;
use common::{Res};
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
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
        let mut ledger = self.ledger.lock().await;
        let contracts = ledger.get_contracts().await.unwrap();
        info!("{:?}", contracts);
        
        Ok(())
    }
}
