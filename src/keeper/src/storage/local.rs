use super::{key_to_path, IStorage};
use async_trait::async_trait;
use common::{types::Bytes, ErrorKind, Res};
use futures::TryStreamExt;
use libp2p::kad::Record;
use log::info;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use std::path::PathBuf;
use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

#[derive(Default)]
pub struct LocalStorage {
    local_storage: LocalFileSystem,
    records: HashMap<String, Record>,
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
    async fn put(&self, data: Record) -> Res<()> {
        let path = key_to_path(&data.key)?;
        info!("storing {}", path.clone().display());
        self.local_storage
            .put(
                &Path::from(path.to_str().ok_or(ErrorKind::InvalidRecordName)?),
                data.value.into(),
            )
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

    // fn sync_put(&self, data: Record) -> Res<()> {
    //     let handle = Handle::current();
    //     match block_on(async { handle.spawn(self.put(data)).await }) {
    //         Ok(Ok(x)) => Ok(x),
    //         Ok(Err(_)) => Err(ErrorKind::AsyncExecutionFailed.into()),
    //         Err(e) => Err(ErrorKind::JoinError(e).into()),
    //     }
    // }
}

impl LocalStorage {
    pub fn new(prefix: &str) -> Res<Self> {
        // match self.records.entry(r.key.clone()) {
        //     hash_map::Entry::Occupied(mut e) => {
        //         e.insert(r);
        //     }
        //     hash_map::Entry::Vacant(e) => {
        //         if num_records >= self.config.max_records {
        //             return Err(Error::MaxRecords);
        //         }
        //         e.insert(r);
        //     }
        let object_store =
            local::LocalFileSystem::new_with_prefix(prefix).map_err(ErrorKind::LocalStorageFail)?;
        Ok(LocalStorage {
            local_storage: object_store,
            records: HashMap::new(),
        })
    }
}
