// use libp2p::kad::record::Key;
// use libp2p::kad::store::{Error, RecordStore, Result};
// use libp2p::kad::{KBucketKey, ProviderRecord, Record, K_VALUE};
// use libp2p_identity::PeerId;
// use smallvec::SmallVec;
// use std::borrow::Cow;
// use std::collections::{hash_map, hash_set, HashMap, HashSet};
// use std::iter;

// /// In-memory implementation of a `RecordStore`.
// pub struct MemoryStore {
//     /// The identity of the peer owning the store.
//     local_key: KBucketKey<PeerId>,
//     /// The configuration of the store.
//     config: MemoryStoreConfig,
//     /// The stored (regular) records.
//     records: HashMap<Key, Record>,
//     /// The stored provider records.
//     providers: HashMap<Key, SmallVec<[ProviderRecord; K_VALUE.get()]>>,
//     /// The set of all provider records for the node identified by `local_key`.
//     ///
//     /// Must be kept in sync with `providers`.
//     provided: HashSet<ProviderRecord>,
// }

// /// Configuration for a `MemoryStore`.
// #[derive(Debug, Clone)]
// pub struct MemoryStoreConfig {
//     /// The maximum number of records.
//     pub max_records: usize,
//     /// The maximum size of record values, in bytes.
//     pub max_value_bytes: usize,
//     /// The maximum number of providers stored for a key.
//     ///
//     /// This should match up with the chosen replication factor.
//     pub max_providers_per_key: usize,
//     /// The maximum number of provider records for which the
//     /// local node is the provider.
//     pub max_provided_keys: usize,
// }

// impl Default for MemoryStoreConfig {
//     fn default() -> Self {
//         Self {
//             max_records: 1024,
//             max_value_bytes: 65 * 1024,
//             max_provided_keys: 1024,
//             max_providers_per_key: K_VALUE.get(),
//         }
//     }
// }

// impl MemoryStore {
//     /// Creates a new `MemoryRecordStore` with a default configuration.
//     pub fn new(local_id: PeerId) -> Self {
//         Self::with_config(local_id, Default::default())
//     }

//     /// Creates a new `MemoryRecordStore` with the given configuration.
//     pub fn with_config(local_id: PeerId, config: MemoryStoreConfig) -> Self {
//         MemoryStore {
//             local_key: KBucketKey::from(local_id),
//             config,
//             records: HashMap::default(),
//             provided: HashSet::default(),
//             providers: HashMap::default(),
//         }
//     }

//     /// Retains the records satisfying a predicate.
//     pub fn retain<F>(&mut self, f: F)
//     where
//         F: FnMut(&Key, &mut Record) -> bool,
//     {
//         self.records.retain(f);
//     }
// }

// #[allow(clippy::all)]
// impl RecordStore for MemoryStore {
//     type RecordsIter<'a> =
//         iter::Map<hash_map::Values<'a, Key, Record>, fn(&'a Record) -> Cow<'a, Record>>;

//     type ProvidedIter<'a> = iter::Map<
//         hash_set::Iter<'a, ProviderRecord>,
//         fn(&'a ProviderRecord) -> Cow<'a, ProviderRecord>,
//     >;

//     fn get(&self, k: &Key) -> Option<Cow<'_, Record>> {
//         self.records.get(k).map(Cow::Borrowed)
//     }

//     fn put(&mut self, r: Record) -> Result<()> {
//         if r.value.len() >= self.config.max_value_bytes {
//             return Err(Error::ValueTooLarge);
//         }

//         let num_records = self.records.len();

//         match self.records.entry(r.key.clone()) {
//             hash_map::Entry::Occupied(mut e) => {
//                 e.insert(r);
//             }
//             hash_map::Entry::Vacant(e) => {
//                 if num_records >= self.config.max_records {
//                     return Err(Error::MaxRecords);
//                 }
//                 e.insert(r);
//             }
//         }

//         Ok(())
//     }

//     fn remove(&mut self, k: &Key) {
//         self.records.remove(k);
//     }

//     fn records(&self) -> Self::RecordsIter<'_> {
//         self.records.values().map(Cow::Borrowed)
//     }

//     fn add_provider(&mut self, record: ProviderRecord) -> Result<()> {
//         let num_keys = self.providers.len();

//         // Obtain the entry
//         let providers = match self.providers.entry(record.key.clone()) {
//             e @ hash_map::Entry::Occupied(_) => e,
//             e @ hash_map::Entry::Vacant(_) => {
//                 if self.config.max_provided_keys == num_keys {
//                     return Err(Error::MaxProvidedKeys);
//                 }
//                 e
//             }
//         }
//         .or_insert_with(Default::default);

//         if let Some(i) = providers.iter().position(|p| p.provider == record.provider) {
//             // In-place update of an existing provider record.
//             providers.as_mut()[i] = record;
//         } else {
//             // It is a new provider record for that key.
//             let local_key = self.local_key.clone();
//             let key = KBucketKey::new(record.key.clone());
//             let provider = KBucketKey::from(record.provider);
//             if let Some(i) = providers.iter().position(|p| {
//                 let pk = KBucketKey::from(p.provider);
//                 provider.distance(&key) < pk.distance(&key)
//             }) {
//                 // Insert the new provider.
//                 if local_key.preimage() == &record.provider {
//                     self.provided.insert(record.clone());
//                 }
//                 providers.insert(i, record);
//                 // Remove the excess provider, if any.
//                 if providers.len() > self.config.max_providers_per_key {
//                     if let Some(p) = providers.pop() {
//                         self.provided.remove(&p);
//                     }
//                 }
//             } else if providers.len() < self.config.max_providers_per_key {
//                 // The distance of the new provider to the key is larger than
//                 // the distance of any existing provider, but there is still room.
//                 if local_key.preimage() == &record.provider {
//                     self.provided.insert(record.clone());
//                 }
//                 providers.push(record);
//             }
//         }
//         Ok(())
//     }

//     fn providers(&self, key: &Key) -> Vec<ProviderRecord> {
//         self.providers
//             .get(key)
//             .map_or_else(Vec::new, |ps| ps.clone().into_vec())
//     }

//     fn provided(&self) -> Self::ProvidedIter<'_> {
//         self.provided.iter().map(Cow::Borrowed)
//     }

//     fn remove_provider(&mut self, key: &Key, provider: &PeerId) {
//         if let hash_map::Entry::Occupied(mut e) = self.providers.entry(key.clone()) {
//             let providers = e.get_mut();
//             if let Some(i) = providers.iter().position(|p| &p.provider == provider) {
//                 let p = providers.remove(i);
//                 self.provided.remove(&p);
//             }
//             if providers.is_empty() {
//                 e.remove();
//             }
//         }
//     }
// }
