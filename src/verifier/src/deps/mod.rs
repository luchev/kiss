use crate::{
    ledger::{ImmuLedger, LedgerProvider},
    settings::{ISettings, SettingsProvider}, grpc::{keeper_client::{KeeperGatewayProvider, IKeeperGateway, KeeperGateway}, GrpcHandlerProvider, IGrpcHandler},
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

    Ok(injector.build())
}
