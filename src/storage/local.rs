use super::{key_to_path, IStorage};
use crate::util::{types::Bytes, ErrorKind, Res};
use async_trait::async_trait;
use futures::TryStreamExt;
use libp2p::kad::Record;
use libp2p_identity::PeerId;
use log::info;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use serde::{
    de::{self, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{borrow::BorrowMut, collections::HashMap, error::Error, time::Instant};
use std::{path::PathBuf, time::Duration};

#[derive(Default)]
pub struct LocalStorage {
    local_storage: LocalFileSystem,
    records: HashMap<String, Record>,
}

struct RecordWrapper(Record);

struct PeerIdWrapper(libp2p::PeerId);

impl Serialize for RecordWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized = serializer.serialize_struct("Record", 4)?;

        let publisher = match &self.0.publisher {
            Some(peer_id) => Some(PeerIdWrapper(peer_id.clone())),
            None => None,
        };

        let expires = match &self.0.expires {
            Some(expires) => Some(expires.elapsed()),
            None => None,
        };

        serialized.serialize_field(
            "key",
            &String::from_utf8(self.0.key.to_vec()).unwrap_or_default(),
        )?;
        serialized.serialize_field("value", &self.0.value)?;
        serialized.serialize_field("publisher", &publisher)?;
        serialized.serialize_field("expires", &expires)?;
        serialized.end()
    }
}

// pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let duration = Duration::deserialize(deserializer)?;
//     let now = Instant::now();
//     let instant = now
//         .checked_sub(duration)
//         .ok_or_else(|| Error::custom("Erreur checked_add"))?;
//     Ok(instant)
// }

impl Serialize for PeerIdWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized = serializer.serialize_struct("PeerId", 1)?;
        serialized.serialize_field("peer_id", &self.0.to_base58())?;
        serialized.end()
    }
}

#[async_trait]
impl IStorage for LocalStorage {
    async fn put(&self, data: Record) -> Res<()> {
        let path = key_to_path(&data.key)?;
        info!("storing {}", path.clone().display());
        let data = RecordWrapper(data);
        let serialized_data =
            serde_yaml::to_string(&data).map_err(ErrorKind::StoragePutSerdeError)?;
        self.local_storage
            .put(
                &Path::from(path.to_str().ok_or(ErrorKind::InvalidRecordName)?),
                serialized_data.into(),
            )
            .await
            .map_err(ErrorKind::StoragePutFailed)?;
        Ok(())
    }

    async fn get(&self, path: PathBuf) -> Res<Bytes> {
        let path = path
            .to_str()
            .ok_or_else(|| ErrorKind::PathParsingError(path.clone()))?;
        info!("retrieving {}", path);
        let bytes = self
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
            .collect::<Bytes>();
        // let record = serde_yaml::from_slice(&bytes).map_err(ErrorKind::StorageGetSerdeError)?;
        Ok(bytes)
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
