use crate::util::{ErrorKind, Res};
use futures::executor::block_on;
use libp2p::kad::record::Key;
use libp2p::kad::store::{Error, RecordStore, Result};
use libp2p::kad::{KBucketKey, ProviderRecord, Record, K_VALUE};
use libp2p_identity::PeerId;
use log::{debug, info, warn};
use runtime_injector::Svc;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::collections::{hash_map, hash_set, HashMap, HashSet};
use std::path::PathBuf;
use std::{iter, str};
use tokio::runtime::Handle;

use crate::storage::IStorage;

/// Local implementation of a `RecordStore`.
pub struct LocalStore {
    /// The identity of the peer owning the store.
    local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: LocalStoreConfig,
    /// The stored (regular) records.
    storage: Svc<dyn IStorage>,
    /// The stored provider records.
    providers: HashMap<Key, SmallVec<[ProviderRecord; K_VALUE.get()]>>,
    /// The set of all provider records for the node identified by `local_key`.
    ///
    /// Must be kept in sync with `providers`.
    provided: HashSet<ProviderRecord>,

    records: HashMap<Key, Record>,
}

/// Configuration for a `LocalStore`.
#[derive(Debug, Clone)]
pub struct LocalStoreConfig {
    /// The maximum number of records.
    pub max_records: usize,
    /// The maximum size of record values, in bytes.
    pub max_value_bytes: usize,
    /// The maximum number of providers stored for a key.
    ///
    /// This should match up with the chosen replication factor.
    pub max_providers_per_key: usize,
    /// The maximum number of provider records for which the
    /// local node is the provider.
    pub max_provided_keys: usize,
}

impl Default for LocalStoreConfig {
    fn default() -> Self {
        Self {
            max_records: 1024,
            max_value_bytes: 65 * 1024,
            max_provided_keys: 1024,
            max_providers_per_key: K_VALUE.get(),
        }
    }
}

impl LocalStore {
    /// Creates a new `LocalRecordStore` with the given configuration.
    pub fn with_config(
        local_id: PeerId,
        config: LocalStoreConfig,
        storage: Svc<dyn IStorage>,
    ) -> Self {
        LocalStore {
            local_key: KBucketKey::from(local_id),
            config,
            storage,
            provided: HashSet::default(),
            providers: HashMap::default(),
            records: HashMap::default(),
        }
    }
}

fn key_to_path(key: &Key) -> Res<PathBuf> {
    Ok(PathBuf::from(
        str::from_utf8(&key.to_vec()).map_err(|e| ErrorKind::Utf8Error)?,
    ))
}

impl RecordStore for LocalStore {
    type RecordsIter<'a> =
        iter::Map<hash_map::Values<'a, Key, Record>, fn(&'a Record) -> Cow<'a, Record>>;

    type ProvidedIter<'a> = iter::Map<
        hash_set::Iter<'a, ProviderRecord>,
        fn(&'a ProviderRecord) -> Cow<'a, ProviderRecord>,
    >;

    fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
        warn!("get {:?}", k);
        let local = match key_to_path(k) {
            Ok(path) => {
                let handle = Handle::current();
                let records = self.storage.clone();
                match block_on(async { handle.spawn(async move { records.get(path).await }).await })
                {
                    Ok(Ok(x)) => Some(Cow::Owned(Record {
                        key: k.clone(),
                        value: x,
                        publisher: None,
                        expires: None,
                    })),
                    _ => None,
                }
            }
            Err(_) => None,
        };
        let memory = self.records.get(k).map(Cow::Borrowed);

        local
    }

    fn put(&mut self, r: Record) -> Result<()> {
        let record = r.clone();
        let memory: Result<()> = {
            if r.value.len() >= self.config.max_value_bytes {
                return Err(Error::ValueTooLarge);
            }

            let num_records = self.records.len();

            match self.records.entry(record.key.clone()) {
                hash_map::Entry::Occupied(mut e) => {
                    e.insert(record);
                }
                hash_map::Entry::Vacant(e) => {
                    if num_records >= self.config.max_records {
                        return Err(Error::MaxRecords);
                    }
                    e.insert(record);
                }
            }

            Ok(())
        };

        let local = {
            if r.value.len() >= self.config.max_value_bytes {
                return Err(Error::ValueTooLarge);
            }
            debug!("put {:?}", r.key.clone());
            let handle = Handle::current();
            let records = self.storage.clone();
            match block_on(async { handle.spawn(async move { records.put(r).await }).await }) {
                Ok(Ok(x)) => Ok(x),
                _ => Err(Error::MaxRecords),
            }
        };

        local
    }

    fn remove(&mut self, k: &Key) {
        warn!("remove {:?}", k);
        // self.records.remove(k);
    }

    fn records(&self) -> Self::RecordsIter<'_> {
        warn!("records");
        todo!()
        // self.records.values().map(Cow::Borrowed)
    }

    fn add_provider(&mut self, record: ProviderRecord) -> Result<()> {
        warn!("add_provider {:?}", record);
        // let num_keys = self.providers.len();

        // // Obtain the entry
        // let providers = match self.providers.entry(record.key.clone()) {
        //     e @ hash_map::Entry::Occupied(_) => e,
        //     e @ hash_map::Entry::Vacant(_) => {
        //         if self.config.max_provided_keys == num_keys {
        //             return Err(Error::MaxProvidedKeys);
        //         }
        //         e
        //     }
        // }
        // .or_insert_with(Default::default);

        // if let Some(i) = providers.iter().position(|p| p.provider == record.provider) {
        //     // In-place update of an existing provider record.
        //     match providers.get_mut(i) {
        //         Some(x) => *x = record,
        //         None => todo!(),
        //     };
        // } else {
        //     // It is a new provider record for that key.
        //     let local_key = self.local_key.clone();
        //     let key = KBucketKey::new(record.key.clone());
        //     let provider = KBucketKey::from(record.provider);
        //     if let Some(i) = providers.iter().position(|p| {
        //         let pk = KBucketKey::from(p.provider);
        //         provider.distance(&key) < pk.distance(&key)
        //     }) {
        //         // Insert the new provider.
        //         if local_key.preimage() == &record.provider {
        //             self.provided.insert(record.clone());
        //         }
        //         providers.insert(i, record);
        //         // Remove the excess provider, if any.
        //         if providers.len() > self.config.max_providers_per_key {
        //             if let Some(p) = providers.pop() {
        //                 self.provided.remove(&p);
        //             }
        //         }
        //     } else if providers.len() < self.config.max_providers_per_key {
        //         // The distance of the new provider to the key is larger than
        //         // the distance of any existing provider, but there is still room.
        //         if local_key.preimage() == &record.provider {
        //             self.provided.insert(record.clone());
        //         }
        //         providers.push(record);
        //     }
        // }
        Ok(())
    }

    fn providers(&self, key: &Key) -> Vec<ProviderRecord> {
        warn!("providers {:?}", key);
        self.providers
            .get(key)
            .map_or_else(Vec::new, |ps| ps.clone().into_vec())
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        warn!("provided");
        self.provided.iter().map(Cow::Borrowed)
    }

    fn remove_provider(&mut self, key: &Key, provider: &PeerId) {
        warn!("remove_provider {:?} {:?}", key, provider);
        if let hash_map::Entry::Occupied(mut e) = self.providers.entry(key.clone()) {
            let providers = e.get_mut();
            if let Some(i) = providers.iter().position(|p| &p.provider == provider) {
                let p = providers.remove(i);
                self.provided.remove(&p);
            }
            if providers.is_empty() {
                e.remove();
            }
        }
    }
}
