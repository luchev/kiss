use std::path::PathBuf;

use async_trait::async_trait;
use common::errors::Result;
pub mod local;
use runtime_injector::{interface, Service};

use crate::types::Bytes;

use self::local::LocalStorage;

#[async_trait]
pub trait Storage: Service {
    async fn put(&self, path: PathBuf, data: Bytes) -> Result<()>;
    async fn get(&self, path: PathBuf) -> Result<Bytes>;
}

interface! {
    dyn Storage = [
        LocalStorage,
    ]
}
