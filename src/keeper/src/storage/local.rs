use super::IStorage;
use crate::types::Bytes;
use async_trait::async_trait;
use common::{ErrorKind, Res};
use futures::TryStreamExt;
use log::info;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

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
        let path = path
            .to_str()
            .ok_or_else(|| ErrorKind::PathParsingError(path.clone()))?;
        info!("storing {}", path);
        self.local_storage
            .put(&Path::from(path), data.into())
            .await
            .map_err(|err| ErrorKind::StoragePutFailed(err).into())
    }

    async fn get(&self, path: PathBuf) -> Res<Bytes> {
        let path = path
            .to_str()
            .ok_or_else(|| ErrorKind::PathParsingError(path.clone()))?;
        info!("retrieving {}", path);
        Ok(self
            .local_storage
            .get(&Path::from(path))
            .await
            .map_err(ErrorKind::StoragePutFailed)?
            .into_stream()
            .map_err(ErrorKind::StorageConvertToStreamFailed)
            .map_ok(Bytes::from)
            .try_collect::<Vec<Bytes>>()
            .await?
            .into_iter()
            .flatten()
            .collect::<Bytes>())
    }
}

impl LocalStorage {
    pub fn new(prefix: &str) -> Res<Self> {
        let object_store =
            local::LocalFileSystem::new_with_prefix(prefix).map_err(ErrorKind::LocalStorageFail)?;
        Ok(LocalStorage {
            local_storage: object_store,
        })
    }
}
