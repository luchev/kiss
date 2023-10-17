use super::{key_to_path, IStorage};
use async_trait::async_trait;
use common::{types::Bytes, ErrorKind, Res};
use futures::TryStreamExt;
use libp2p::kad::RecordKey;
use libp2p::kad::{KBucketKey, Record};
use libp2p_identity::PeerId;
use log::info;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
pub struct LocalStorage {
    local_storage: LocalFileSystem,
    records: HashMap<String, Record>,
}

struct RecordWrapper(Record);

// impl Serialize for RecordWrapper {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         let mut serialized = serializer.serialize_struct("Record", 4)?;

//         serialized.serialize_field("key", &self.0.key)?;
//         serialized.serialize_field("value", &self.0.value)?;
//         serialized.serialize_field("publisher", &self.0.publisher)?;
//         serialized.serialize_field("expires", &self.0.expires)?;
//         serialized.end()
//     }
// }

// impl<'de> Deserialize<'de> for RecordWrapper {
//     fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error> {
//         todo!()
//     }
// }

#[async_trait]
impl IStorage for LocalStorage {
    async fn put(&self, data: Record) -> Res<()> {
        let path = key_to_path(&data.key)?;
        info!("storing {}", path.clone().display());
        // self.local_storage
        //     .put(
        //         &Path::from(path.to_str().ok_or(ErrorKind::InvalidRecordName)?),
        //         serde_yaml::to_string(&RecordWrapper(&data))
        //             .map_err(|err| ErrorKind::StoragePutFailed(err))
        //             .into(),
        //     )
        //     .await
        //     .map_err(|err| ErrorKind::StoragePutFailed(err).into())
        Ok(())
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
