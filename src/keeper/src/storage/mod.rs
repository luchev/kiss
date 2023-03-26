use std::path::PathBuf;

use async_trait::async_trait;
use common::Res;
pub mod local;
use runtime_injector::{interface, Service};

use crate::types::Bytes;

use self::local::LocalStorage;

#[async_trait]
pub trait IStorage: Service {
    async fn put(&self, path: PathBuf, data: Bytes) -> Res<()>;
    async fn get(&self, path: PathBuf) -> Res<Bytes>;
}

interface! {
    dyn IStorage = [
        LocalStorage,
    ]
}
