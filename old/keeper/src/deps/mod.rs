use crate::{
    grpc::{GrpcProvider, IGrpcHandler},
    p2p,
    settings::{ISettings, SettingsProvider},
    storage::{IStorage, StorageProvider},
};
use common::Res;
use runtime_injector::{Injector, IntoSingleton, TypedProvider};

pub fn dependency_injector() -> Res<Injector> {
    let mut injector = Injector::builder();
    injector.add_module(p2p::module());
    injector.provide(StorageProvider.singleton().with_interface::<dyn IStorage>());
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

    Ok(injector.build())
}
