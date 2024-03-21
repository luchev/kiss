use crate::ledger::{ImmuLedger, LedgerProvider};
use crate::malice::{IMalice, MaliceProvider};
use crate::util::Res;
use crate::verifier::{IVerifier, VerifierProvider};
use crate::{
    grpc::{GrpcProvider, IGrpcHandler},
    p2p,
    settings::{ISettings, SettingsProvider},
    storage::{IStorage, StorageProvider},
};
use runtime_injector::{Injector, IntoSingleton, TypedProvider};
use tokio::sync::Mutex;

pub fn dependency_injector() -> Res<Injector> {
    let mut injector = Injector::builder();
    injector.add_module(p2p::module());
    injector.provide(StorageProvider.singleton().with_interface::<dyn IStorage>());
    injector.provide(
        MaliceProvider
            .singleton()
            .with_interface::<Box<dyn IMalice>>(),
    );
    injector.provide(
        SettingsProvider
            .singleton()
            .with_interface::<dyn ISettings>(),
    );
    injector.provide(
        GrpcProvider
            .singleton()
            .with_interface::<dyn IGrpcHandler>(),
    );
    injector.provide(
        LedgerProvider
            .singleton()
            .with_interface::<Mutex<ImmuLedger>>(),
    );
    injector.provide(
        VerifierProvider
            .singleton()
            .with_interface::<dyn IVerifier>(),
    );

    Ok(injector.build())
}
