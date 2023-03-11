use crate::{settings::Settings, storage::Storage, types::Bytes};
use async_trait::async_trait;
use common::errors::{ErrorKind, Result};
use futures::StreamExt;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use runtime_injector::{Arg};
use std::path::PathBuf;

pub struct LocalStorage(pub Arg<Settings>);

pub struct Inner {
    local_storage: LocalFileSystem,
}

#[async_trait]
impl Storage for LocalStorage {
    async fn put(&self, path: PathBuf, data: Bytes) -> Result<()> {
        Ok(())
        // self.1
        //     .local_storage
        //     .put(&Path::from(path.to_str().unwrap()), data.into())
        //     .await
        //     .map_err(|err| ErrorKind::StoragePutFailed(err).into())
    }

    async fn get(&self, path: PathBuf) -> Result<Bytes> {
        Ok(vec![].into())
        // Ok(self
        //     .1
        //     .local_storage
        //     .get(&Path::from(path.to_str().unwrap()))
        //     .await
        //     .map_err(|err| ErrorKind::StoragePutFailed(err))?
        //     .into_stream()
        //     .map(|x| x.unwrap().into())
        //     .collect::<Vec<Bytes>>()
        //     .await
        //     .into_iter()
        //     .flatten()
        //     .collect::<Bytes>())
    }
}

impl Inner {
    pub fn new(settings: Settings) -> Result<Self> {
        let prefix = "data";
        Inner::new_with_prefix(prefix)
    }

    fn new_with_prefix(prefix: &str) -> Result<Self> {
        let object_store = local::LocalFileSystem::new_with_prefix(prefix)
            .map_err(|err| ErrorKind::LocalStorageFail(err))?;
        Ok(Inner {
            local_storage: object_store,
        })
    }
}
