use crate::{
    grpc::{GrpcProvider, GrpcProviderImpl},
    settings::{Settings, SettingsImpl, Storage as StorageSettings},
    storage::{local::LocalStorage, Storage},
};
use common::errors::Result;
use runtime_injector::{Injector, IntoSingleton, TypedProvider};

pub fn dependency_injector() -> Result<Injector> {
    let settings = SettingsImpl::new()?;
    let mut builder = Injector::builder();
    match settings.clone().storage {
        StorageSettings::Local { path, create: _create } => {
            builder.provide(
                LocalStorage::constructor(path)
                    .singleton()
                    .with_interface::<dyn Storage>(),
            );
        }
        StorageSettings::Docker => todo!(),
    }

    builder.provide(
        SettingsImpl::constructor()
            .singleton()
            .with_interface::<dyn Settings>(),
    );

    builder.provide(
        GrpcProviderImpl
            .singleton()
            .with_interface::<dyn GrpcProvider>(),
    );

    Ok(builder.build())
}
