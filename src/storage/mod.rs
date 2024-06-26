use crate::util::{Er, ErrorKind, Res};
use async_trait::async_trait;
use libp2p::kad::Record;
use object_store::path::Path;
use std::path::PathBuf;
pub mod local;
use self::local::LocalStorage;
use crate::settings::{ISettings, Storage as StorageSettings};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};

use libp2p::kad::record::Key;
use std::str;

#[async_trait]
pub trait IStorage: Service {
    async fn put(&self, data: Record) -> Res<()>;
    async fn get(&self, path: PathBuf) -> Res<Record>;
    async fn remove(&self, path: &Path) -> Res<()>;
    async fn list(&self) -> Res<Vec<Path>>;
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
            StorageSettings::Local { path, create } => Ok(LocalStorage::new(path, create)
                .map_err(|err| InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<LocalStorage>(),
                    inner: Box::<Er>::new(err),
                })?),
            StorageSettings::Docker => todo!(),
        }
    }
}

fn key_to_path(key: &Key) -> Res<PathBuf> {
    Ok(PathBuf::from(
        str::from_utf8(&key.to_vec()).map_err(|_e| ErrorKind::Utf8Error)?,
    ))
}

interface! {
    dyn IStorage = [
        LocalStorage,
    ]
}
