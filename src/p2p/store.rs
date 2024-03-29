use crate::util::{ErrorKind, Res};
use futures::executor::block_on;
use libp2p::kad::record::Key;
use libp2p::kad::store::{Error, RecordStore, Result};
use libp2p::kad::{KBucketKey, ProviderRecord, Record, K_VALUE};
use libp2p_identity::PeerId;
use log::debug;
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
    _local_key: KBucketKey<PeerId>,
    /// The configuration of the store.
    config: LocalStoreConfig,
    /// The stored (regular) records.
    storage: Svc<dyn IStorage>,
    /// The stored provider records.
    _providers: HashMap<Key, SmallVec<[ProviderRecord; K_VALUE.get()]>>,
    /// The set of all provider records for the node identified by `local_key`.
    ///
    /// Must be kept in sync with `providers`.
    _provided: HashSet<ProviderRecord>,
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
            _local_key: KBucketKey::from(local_id),
            config,
            storage,
            _provided: HashSet::default(),
            _providers: HashMap::default(),
        }
    }
}

fn key_to_path(key: &Key) -> Res<PathBuf> {
    Ok(PathBuf::from(
        str::from_utf8(&key.to_vec()).map_err(|_e| ErrorKind::Utf8Error)?,
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
        match key_to_path(k) {
            Ok(path) => {
                let handle = Handle::current();
                let records = self.storage.clone();
                match block_on(async { handle.spawn(async move { records.get(path).await }).await })
                {
                    Ok(Ok(x)) => Some(Cow::Owned(x)),
                    _ => None,
                }
            }
            Err(_) => None,
        }
    }

    fn put(&mut self, r: Record) -> Result<()> {
        if r.value.len() >= self.config.max_value_bytes {
            return Err(Error::ValueTooLarge);
        }
        let handle = Handle::current();
        let records = self.storage.clone();
        match block_on(async { handle.spawn(async move { records.put(r).await }).await }) {
            Ok(Ok(x)) => Ok(x),
            _ => Err(Error::MaxRecords), // not exactly as it could be some storage issue but for now it's ok
        }
    }

    fn remove(&mut self, key: &Key) {
        match key_to_path(key) {
            Ok(path) => {
                let handle = Handle::current();
                let records = self.storage.clone();
                match block_on(async {
                    handle
                        .spawn(async move { records.remove(path).await })
                        .await
                }) {
                    Ok(Ok(x)) => debug!("removed record: {:?}", x),
                    _ => debug!("failed to remove record"),
                }
            }
            Err(_) => debug!("failed to remove record"),
        }
    }

    fn records(&self) -> Self::RecordsIter<'_> {
        todo!();
    }

    fn add_provider(&mut self, _record: ProviderRecord) -> Result<()> {
        todo!();
    }

    fn providers(&self, _key: &Key) -> Vec<ProviderRecord> {
        todo!();
    }

    fn provided(&self) -> Self::ProvidedIter<'_> {
        todo!();
    }

    fn remove_provider(&mut self, _key: &Key, _provider: &PeerId) {
        todo!();
    }
}
