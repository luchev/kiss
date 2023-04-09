use crate::{
    grpc::{GrpcProvider, IGrpcHandler},
    p2p::{ISwarm, SwarmProvider, Instruction},
    settings::{ISettings, SettingsProvider},
    storage::{IStorage, StorageProvider},
};
use common::Res;
use runtime_injector::{Injector, IntoSingleton, TypedProvider, ConstantProvider};
use tokio::sync::{mpsc, Mutex};

pub fn dependency_injector() -> Res<Injector> {
    let mut injector = Injector::builder();
    let (tx, rx) = mpsc::channel::<Instruction>(32);
    injector.provide(ConstantProvider::new(Mutex::new(tx)).with_interface::<Mutex<mpsc::Sender<Instruction>>>());
    injector.provide(ConstantProvider::new(Mutex::new(rx)).with_interface::<Mutex<mpsc::Receiver<Instruction>>>());

    injector.provide(
        SettingsProvider
            .singleton()
            .with_interface::<dyn ISettings>(),
    );
    injector.provide(StorageProvider.singleton().with_interface::<dyn IStorage>());
    injector.provide(SwarmProvider.singleton().with_interface::<dyn ISwarm>());
    injector.provide(
        GrpcProvider
            .singleton()
            .with_interface::<dyn IGrpcHandler>(),
    );

    Ok(injector.build())
}
