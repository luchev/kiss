use crate::{
    grpc::{IGrpcProvider, GrpcProvider},
    settings::{ISettings, Settings, Storage as StorageSettings},
    storage::{local::LocalStorage, IStorage}, p2p::{Swarm, ISwarm},
};
use common::Res;
use runtime_injector::{Injector, IntoSingleton, TypedProvider};

pub fn dependency_injector() -> Res<Injector> {
    let settings = Settings::new();
    let mut builder = Injector::builder();
    // TODO remove duplicated settings initialization

    builder.provide(
        Settings::constructor()
            .singleton()
            .with_interface::<dyn ISettings>(),
    );

    match settings.clone().storage {
        StorageSettings::Local {
            path,
            create: _create,
        } => {
            builder.provide(
                LocalStorage::constructor(path)
                    .singleton()
                    .with_interface::<dyn IStorage>(),
            );
        }
        StorageSettings::Docker => todo!(),
    }

    builder.provide(
        GrpcProvider
            .singleton()
            .with_interface::<dyn IGrpcProvider>(),
    );

    builder.provide(
        Swarm
            .singleton()
            .with_interface::<dyn ISwarm>(),
    );

    Ok(builder.build())
}
