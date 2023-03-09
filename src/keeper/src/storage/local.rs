use crate::{storage::Storage, types::Bytes};
use async_trait::async_trait;
use common::errors::{ErrorKind, Result};
use futures::StreamExt;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use std::path::PathBuf;

pub struct LocalStorage {
    local_storage: LocalFileSystem,
}

#[async_trait]
impl Storage for LocalStorage {
    fn list(&self) {
        println!("Hi");
    }

    async fn put(&self, path: PathBuf, data: Bytes) -> Result<()> {
        self.local_storage
            .put(&Path::from(path.to_str().unwrap()), data.into())
            .await
            .map_err(|err| ErrorKind::StoragePutFailed(err).into())
    }

    async fn get(&self, path: PathBuf) -> Result<Bytes> {
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

impl Default for LocalStorage {
    fn default() -> LocalStorage {
        let prefix = "data";
        LocalStorage::new_with_prefix(prefix).unwrap()
    }
}

impl LocalStorage {
    fn new_with_prefix(prefix: &str) -> Result<Self> {
        let object_store = local::LocalFileSystem::new_with_prefix(prefix)
            .map_err(|err| ErrorKind::LocalStorageFail(err))?;
        Ok(LocalStorage {
            local_storage: object_store,
        })
    }
}

// let prefix: Path = "~".try_into().unwrap();
// object_store
//     .list(Some(&prefix))
//     .await
//     .expect("")
//     .for_each(move |x| async {
//         let x = x.expect("");
//         println!("{:?}", x);
//     })
//     .await;
