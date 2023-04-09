use async_trait::async_trait;
use common::Res;
use std::path::PathBuf;
pub mod local;
use self::local::LocalStorage;
use crate::{
    settings::{ISettings, Storage as StorageSettings},
    types::Bytes,
};
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
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
        let settings = injector.get::<Svc<dyn ISettings>>().unwrap().storage();

        match settings {
            StorageSettings::Local {
                path,
                create: _create,
            } => Ok(LocalStorage::new(path.as_str()).unwrap()),
            StorageSettings::Docker => todo!(),
        }
    }
}

interface! {
    dyn IStorage = [
        LocalStorage,
    ]
}
