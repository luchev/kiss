use super::{key_to_path, IStorage};
use crate::util::{types::Bytes, ErrorKind, Res};
use async_trait::async_trait;
use base64::Engine;
use futures::TryStreamExt;
use libp2p::kad::Record;
use libp2p_identity::PeerId;
use libp2p_kad::RecordKey;
use log::info;
use object_store::{
    local::{self, LocalFileSystem},
    path::Path,
    ObjectStore,
};
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    ser::SerializeStruct,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{collections::HashMap, fmt, time::Instant};
use std::{path::PathBuf, time::Duration};

#[derive(Default)]
pub struct LocalStorage {
    local_storage: LocalFileSystem,
    records: HashMap<String, Record>,
}

struct RecordWrapper(Record);

impl Serialize for RecordWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut serialized = serializer.serialize_struct("Record", 4)?;

        let key = String::from_utf8(self.0.key.to_vec()).unwrap_or_default();
        let value = base64::engine::general_purpose::STANDARD.encode(&self.0.value);
        let publisher = match &self.0.publisher {
            Some(peer_id) => Some(peer_id.to_base58()),
            None => None,
        };
        let expires = match &self.0.expires {
            Some(expires) => Some(expires.elapsed()),
            None => None,
        };

        serialized.serialize_field("key", &key)?;
        serialized.serialize_field("publisher", &publisher)?;
        serialized.serialize_field("expires", &expires)?;
        serialized.serialize_field("value", &value)?;
        serialized.end()
    }
}
impl<'de> Deserialize<'de> for RecordWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "lowercase")]
        enum Field {
            Key,
            Publisher,
            Expires,
            Value,
        }

        struct RecordVisitor;

        impl<'de> Visitor<'de> for RecordVisitor {
            type Value = RecordWrapper;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct Record")
            }

            fn visit_map<V>(self, mut map: V) -> Result<RecordWrapper, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut key: Option<String> = None;
                let mut publisher: Option<String> = None;
                let mut expires: Option<Duration> = None;
                let mut value: Option<String> = None;
                while let Some(yaml_key) = map.next_key()? {
                    match yaml_key {
                        Field::Key => {
                            if key.is_some() {
                                return Err(de::Error::duplicate_field("key"));
                            }
                            key = Some(map.next_value()?);
                        }
                        Field::Publisher => {
                            if publisher.is_some() {
                                return Err(de::Error::duplicate_field("publisher"));
                            }
                            publisher = Some(map.next_value()?);
                        }
                        Field::Expires => {
                            if expires.is_some() {
                                return Err(de::Error::duplicate_field("expires"));
                            }
                            expires = Some(map.next_value()?);
                        }
                        Field::Value => {
                            if value.is_some() {
                                return Err(de::Error::duplicate_field("value"));
                            }
                            value = Some(map.next_value()?);
                        }
                    }
                }
                let key = key.ok_or_else(|| de::Error::missing_field("key"))?;
                let publisher = publisher.ok_or_else(|| de::Error::missing_field("publisher"))?;
                let expires = expires.ok_or_else(|| de::Error::missing_field("expires"))?;
                let value = value.ok_or_else(|| de::Error::missing_field("value"))?;
                Ok(RecordWrapper(Record {
                    key: RecordKey::new(&String::into_bytes(key)),
                    publisher: Some(
                        PeerId::from_bytes(publisher.as_bytes()).map_err(|err| {
                            de::Error::custom(format!("invalid peer id: {}", err))
                        })?,
                    ),
                    expires: Some(Instant::now() + expires),
                    value: base64::engine::general_purpose::STANDARD
                        .decode(value.as_bytes())
                        .unwrap_or_default(),
                }))
            }
        }

        const FIELDS: &[&str] = &["key", "publisher", "expires", "value"];
        deserializer.deserialize_struct("Record", FIELDS, RecordVisitor)
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

    async fn get(&self, path: PathBuf) -> Res<Record> {
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
        let deserialized_record = serde_yaml::from_slice::<RecordWrapper>(&bytes)
            .map_err(ErrorKind::StorageGetSerdeError)?
            .0;
        Ok(deserialized_record)
    }
}

impl LocalStorage {
    pub fn new(prefix: &str) -> Res<Self> {
        let object_store =
            local::LocalFileSystem::new_with_prefix(prefix).map_err(ErrorKind::LocalStorageFail)?;
        Ok(LocalStorage {
            local_storage: object_store,
            records: HashMap::new(),
        })
    }
}
