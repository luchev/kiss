use crate::{
    ledger::{ImmuLedger, LedgerProvider},
    settings::{ISettings, SettingsProvider},
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

    Ok(injector.build())
}
