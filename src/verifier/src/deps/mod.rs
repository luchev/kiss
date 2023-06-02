use crate::{
    grpc::{
        keeper_client::{KeeperGateway, KeeperGatewayProvider},
        GrpcHandlerProvider, IGrpcHandler,
    },
    ledger::{ImmuLedger, LedgerProvider},
    settings::{ISettings, SettingsProvider},
    verifier::{IVerifier, VerifierProvider},
};
use common::Res;
use runtime_injector::{Injector, IntoSingleton, TypedProvider};
use tokio::sync::Mutex;

pub fn dependency_injector() -> Res<Injector> {
    let mut injector = Injector::builder();
    injector.provide(
        SettingsProvider
            .singleton()
            .with_interface::<dyn ISettings>(),
    );
    injector.provide(
        LedgerProvider
            .singleton()
            .with_interface::<Mutex<ImmuLedger>>(),
    );
    injector.provide(
        KeeperGatewayProvider
            .singleton()
            .with_interface::<Mutex<KeeperGateway>>(),
    );
    injector.provide(
        GrpcHandlerProvider
            .singleton()
            .with_interface::<dyn IGrpcHandler>(),
    );
    injector.provide(
        VerifierProvider
            .singleton()
            .with_interface::<dyn IVerifier>(),
    );

    Ok(injector.build())
}
