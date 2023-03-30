use crate::{
    grpc::{GrpcProvider, IGrpcHandler},
    p2p::{ISwarm, SwarmProvider},
    settings::{ISettings, Settings, Storage as StorageSettings},
    storage::{local::LocalStorage, IStorage},
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

    builder.provide(GrpcProvider.singleton().with_interface::<dyn IGrpcHandler>());
    builder.provide(SwarmProvider.singleton().with_interface::<dyn ISwarm>());
    let injector = builder.build();

    Ok(injector)
}
