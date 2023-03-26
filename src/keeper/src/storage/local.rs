use std::path::PathBuf;
use async_trait::async_trait;
use common::{ErrorKind, Res};
use futures::StreamExt;
use log::info;
use object_store::{
    local::{self, LocalFileSystem}, ObjectStore, path::Path,
};
use std::fmt::{Display, Formatter};
use crate::types::Bytes;
use super::IStorage;

#[derive(Default)]
pub struct LocalStorage {
    local_storage: LocalFileSystem,
}

#[derive(Debug)]
struct FooError;

impl std::error::Error for FooError {}
impl Display for FooError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "An error occurred while creating a Foo")
    }
}

#[async_trait]
impl IStorage for LocalStorage {
    async fn put(&self, path: PathBuf, data: Bytes) -> Res<()> {
        info!("storing {}", path.to_str().unwrap());
        self
            .local_storage
            .put(&Path::from(path.to_str().unwrap()), data.into())
            .await
            .map_err(|err| ErrorKind::StoragePutFailed(err).into())
    }

    async fn get(&self, path: PathBuf) -> Res<Bytes> {
        info!("retrieving {}", path.to_str().unwrap());
        Ok(self
            .local_storage
            .get(&Path::from(path.to_str().unwrap()))
            .await
            .map_err(|err| ErrorKind::StoragePutFailed(err))?
            .into_stream()
            .map(|x| x.unwrap().into())
            .collect::<Vec<Bytes>>()
            .await
            .into_iter()
            .flatten()
            .collect::<Bytes>())
    }
}

impl LocalStorage {
    pub fn constructor(path: String) -> impl Fn() -> LocalStorage {
        move || {
            LocalStorage::new_with_prefix(path.as_str()).unwrap()
        }
    }

    fn new_with_prefix(prefix: &str) -> Res<Self> {
        let object_store = local::LocalFileSystem::new_with_prefix(prefix)
            .map_err(|err| ErrorKind::LocalStorageFail(err))?;
        Ok(LocalStorage {
            local_storage: object_store,
        })
    }
}
