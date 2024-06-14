use crate::settings::{ISettings, Storage as StorageSettings};
use crate::util::{Er, ErrorKind, Res};
use async_trait::async_trait;
use libp2p::kad::Record;
use object_store::path::Path;
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::{collections::HashMap, path::PathBuf, time::SystemTime};
use tokio::sync::Mutex;

#[async_trait]
pub trait IBench: Service {}

#[derive(Default)]
pub struct Bench {
    pub deleted_files: HashMap<String, SystemTime>,
}

impl IBench for Bench {}

pub struct BenchProvider;
impl ServiceFactory<()> for BenchProvider {
    type Result = Mutex<Bench>;

    fn invoke(
        &mut self,
        _injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        Ok(Mutex::new(Bench {
            deleted_files: HashMap::new(),
        }))
    }
}

interface! {
    dyn IBench = [
        Bench,
    ]
}
