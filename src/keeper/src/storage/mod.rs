use async_trait::async_trait;
use common::{types::Bytes, Er, Res};
use std::path::PathBuf;
pub mod local;
use self::local::LocalStorage;
use crate::settings::{ISettings, Storage as StorageSettings};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};

#[async_trait]
pub trait IStorage: Service {
    async fn put(&self, path: PathBuf, data: Bytes) -> Res<()>;
    async fn get(&self, path: PathBuf) -> Res<Bytes>;
}

pub struct StorageProvider;
impl ServiceFactory<()> for StorageProvider {
    type Result = LocalStorage;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>()?.storage();

        match settings {
            StorageSettings::Local {
                path,
                create: _create,
            } => Ok(LocalStorage::new(path.as_str()).map_err(|err| {
                InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<LocalStorage>(),
                    inner: Box::<Er>::new(err),
                }
            })?),
            StorageSettings::Docker => todo!(),
        }
    }
}

interface! {
    dyn IStorage = [
        LocalStorage,
    ]
}
