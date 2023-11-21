use crate::settings::ISettings;
use crate::{
    p2p::memorystore::{MemoryStore, MemoryStoreConfig},
    util::{
        types::{Bytes, OneReceiver, OneSender, SwarmInstruction},
        Er, ErrorKind, Res,
    },
};
use async_trait::async_trait;
use base64::Engine as _;
use futures::StreamExt;
use libp2p::{
    core::upgrade::Version,
    kad::{
        record::Key, GetRecordOk, GetRecordResult, Kademlia, KademliaConfig, KademliaEvent,
        PeerRecord, PutRecordResult, QueryId, QueryResult, Quorum, Record,
    },
    mdns::{self, tokio::Behaviour},
    noise,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::tokio::Transport,
    yamux, PeerId, Transport as _,
};
use libp2p_identity::Keypair;
use libp2p_kad::{
    AddProviderOk, AddProviderResult, GetClosestPeersOk, GetClosestPeersResult, GetProvidersOk,
    GetProvidersResult, RoutingUpdate,
};
use log::{info, warn};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    time::Duration,
};
use tokio::{
    select,
    sync::{mpsc::Receiver, oneshot, Mutex, MutexGuard},
};

interface! {
    dyn ISwarm = [
        Swarm,
    ]
}

pub struct SwarmProvider;

impl ServiceFactory<()> for SwarmProvider {
    type Result = Swarm;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings: Svc<dyn ISettings> = injector.get()?;
        let receiver: Svc<Mutex<Receiver<SwarmInstruction>>> = injector.get()?;
        // let storage: Svc<dyn IStorage> = injector.get()?;

        let local_key = match settings.swarm().keypair {
            Some(keypair) => Keypair::from_protobuf_encoding(
                &base64::engine::general_purpose::STANDARD_NO_PAD
                    .decode(keypair)
                    .map_err(|e| InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<Swarm>(),
                        inner: Box::<Er>::new(ErrorKind::KeypairBase64DecodeError(e).into()),
                    })?,
            )
            .map_err(|e| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<Swarm>(),
                inner: Box::<Er>::new(ErrorKind::KeypairBase64DecodingError(e).into()),
            })?,
            None => generate_keypair(),
        };

        let local_peer_id = PeerId::from(local_key.public());
        info!("starting peer with id: {}", local_peer_id);

        let mut swarm = {
            let cfg = KademliaConfig::default()
                .set_query_timeout(Duration::from_secs(60))
                .to_owned();
            let store = MemoryStore::with_config(
                local_peer_id,
                MemoryStoreConfig {
                    max_records: 150000,
                    max_value_bytes: 1024 * 1024 * 200,
                    max_provided_keys: 150000,
                    max_providers_per_key: 5,
                },
            );

            // let store = LocalStore::with_config(
            //     local_peer_id,
            //     LocalStoreConfig {
            //         max_records: 150000,
            //         max_value_bytes: 1024 * 1024 * 200,
            //         max_provided_keys: 150000,
            //         max_providers_per_key: 20,
            //     },
            //     storage,
            // );
            let mdns = Behaviour::new(mdns::Config::default(), local_peer_id).map_err(|e| {
                InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<Swarm>(),
                    inner: Box::<Er>::new(ErrorKind::BehaviourInitFailed(e).into()),
                }
            })?;

            let kademlia = Kademlia::with_config(local_peer_id, store, cfg);
            let behaviour = CombinedBehaviour { kademlia, mdns };
            let transport = Transport::default()
                .upgrade(Version::V1)
                .authenticate(noise::Config::new(&local_key).map_err(|e| {
                    InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<Swarm>(),
                        inner: Box::<Er>::new(ErrorKind::NoiseInitFailed(e).into()),
                    }
                })?)
                .multiplex(yamux::Config::default())
                .boxed();
            SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
        };

        swarm
            .listen_on(
                format!("/ip4/0.0.0.0/tcp/{}", settings.swarm().port)
                    .parse()
                    .map_err(|e| InjectError::ActivationFailed {
                        service_info: ServiceInfo::of::<Swarm>(),
                        inner: Box::<Er>::new(ErrorKind::IpParseFailed(e).into()),
                    })?,
            )
            .map_err(|e| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<Swarm>(),
                inner: Box::<Er>::new(ErrorKind::SwarmListenFailed(e).into()),
            })?;

        Ok(Swarm {
            inner: Mutex::new(swarm),
            swarm_controller_api: receiver,
            queries: Mutex::new(HashMap::new()),
        })
    }
}

#[async_trait]
pub trait ISwarm: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Swarm {
    inner: Mutex<libp2p::Swarm<CombinedBehaviour>>,
    swarm_controller_api: Svc<Mutex<Receiver<SwarmInstruction>>>,
    queries: Mutex<HashMap<QueryId, QueryResponse>>,
}

#[derive(Debug)]
enum QueryResponse {
    Put {
        sender: OneSender<Res<()>>,
    },
    PutTo {
        sender: OneSender<Res<()>>,
    },
    Get {
        sender: OneSender<Res<Bytes>>,
    },
    GetProviders {
        sender: OneSender<Res<HashSet<PeerId>>>,
    },
    StartProviding {
        sender: OneSender<Res<()>>,
    },
    GetClosestPeers {
        sender: OneSender<Res<Vec<PeerId>>>,
    },
}

#[async_trait]
impl ISwarm for Swarm {
    async fn start(&self) -> Res<()> {
        let mut swarm = self.inner.lock().await;
        let mut receiver = self.swarm_controller_api.lock().await;
        loop {
            select! {
                instruction = receiver.recv() => {
                    self.handle_controller_event(instruction, &mut swarm).await?;
                },
                event = swarm.select_next_some() => {
                    self.handle_swarm_event(event, &mut swarm).await?;
                }
            }
        }
    }
}

impl Swarm {
    async fn handle_swarm_event<'t, SwarmError: Debug>(
        &self,
        event: SwarmEvent<WireEvent, SwarmError>,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
    ) -> Res<()> {
        info!("swarm event {:?}", event);
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("listening on {address:?}");
            }
            SwarmEvent::Behaviour(WireEvent::Mdns(mdns::Event::Discovered(list))) => {
                info!("discovered peers: {list:?}");
                for (peer_id, multiaddr) in list {
                    swarm
                        .behaviour_mut()
                        .kademlia
                        .add_address(&peer_id, multiaddr);
                }
            }
            SwarmEvent::Behaviour(WireEvent::Kademlia(event)) => match event {
                KademliaEvent::OutboundQueryProgressed { result, id, .. } => match result {
                    QueryResult::GetRecord(message) => self.handle_get_record(message, id).await?,
                    QueryResult::PutRecord(result) => self.handle_put_record(result, id).await?,
                    QueryResult::GetProviders(message) => {
                        self.handle_get_providers(message, id).await?
                    }
                    QueryResult::GetClosestPeers(message) => {
                        self.handle_get_closest_peers(message, id).await?
                    }
                    QueryResult::StartProviding(message) => {
                        self.handle_start_providing(message, id).await?
                    }
                    _ => warn!("unhandled query result: {:?}", result),
                },
                KademliaEvent::RoutingUpdated { .. } => {
                    info!("routing updated",);
                }
                _ => warn!("unhandled kademlia event: {:?}", event),
            },
            _ => {}
        }
        Ok(())
    }

    async fn handle_start_providing(&self, message: AddProviderResult, id: QueryId) -> Res<()> {
        let response_channel = match self.queries.lock().await.remove(&id) {
            Some(QueryResponse::StartProviding { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannel(id))?,
        };

        match message {
            Ok(AddProviderOk { .. }) => response_channel.send(Ok(()))?,
            Err(err) => Err(ErrorKind::SwarmStartProvidingError(err))?,
        };
        Ok(())
    }

    async fn handle_get_closest_peers(
        &self,
        message: GetClosestPeersResult,
        id: QueryId,
    ) -> Res<()> {
        let response_channel = match self.queries.lock().await.remove(&id) {
            Some(QueryResponse::GetClosestPeers { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannel(id))?,
        };

        match message {
            Ok(GetClosestPeersOk { peers, .. }) => response_channel.send(Ok(peers))?,
            Err(err) => Err(ErrorKind::SwarmGetClosestPeersError(err))?,
        };
        Ok(())
    }

    async fn handle_get_providers(&self, message: GetProvidersResult, id: QueryId) -> Res<()> {
        let response_channel = match self.queries.lock().await.remove(&id) {
            Some(QueryResponse::GetProviders { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannel(id))?,
        };

        match message {
            Ok(GetProvidersOk::FoundProviders { providers, .. }) => {
                response_channel.send(Ok(providers))?
            }
            Ok(GetProvidersOk::FinishedWithNoAdditionalRecord { .. }) => {
                response_channel.send(Err(ErrorKind::NoProvidersFound.into()))?
            }
            Err(err) => Err(ErrorKind::SwarmGetProvidersError(err))?,
        };
        Ok(())
    }

    async fn handle_get_record(&self, message: GetRecordResult, id: QueryId) -> Res<()> {
        let response_channel = match self.queries.lock().await.remove(&id) {
            Some(QueryResponse::Get { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannel(id))?,
        };

        match message {
            Ok(GetRecordOk::FoundRecord(PeerRecord {
                record: Record { value, .. },
                ..
            })) => response_channel.send(Ok(value))?,
            Ok(_) => response_channel.send(Err(ErrorKind::SwarmGetRecordUnknownError(
                "unexpected GetRecord result".to_string(),
            )
            .into()))?,
            Err(err) => response_channel.send(Err(ErrorKind::SwarmGetRecordError(err).into()))?,
        };
        Ok(())
    }

    async fn handle_put_record(&self, message: PutRecordResult, id: QueryId) -> Res<()> {
        let response_channel = match self.queries.lock().await.remove(&id) {
            Some(QueryResponse::Put { sender }) => sender,
            Some(QueryResponse::PutTo { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannel(id))?,
        };

        match message {
            Ok(_) => response_channel.send(Ok(()))?,
            Err(err) => response_channel.send(Err(ErrorKind::SwarmPutRecordError(err).into()))?,
        };
        Ok(())
    }

    async fn handle_controller_event<'a>(
        &self,
        instruction: Option<SwarmInstruction>,
        swarm: &mut MutexGuard<'a, libp2p::Swarm<CombinedBehaviour>>,
    ) -> Res<()> {
        let instruction = instruction.ok_or(ErrorKind::MissingInstruction)?;
        info!("instruction {:?}", instruction);
        match instruction {
            SwarmInstruction::PutLocal { key, value, resp } => {
                self.handle_controller_put(swarm, key, value, resp).await
            }
            SwarmInstruction::PutRemote {
                key,
                value,
                remotes,
                resp,
            } => {
                self.handle_controller_put_record_to(swarm, key, value, remotes, resp)
                    .await
            }
            SwarmInstruction::Get { key, resp } => {
                self.handle_controller_get(swarm, key, resp).await
            }
            SwarmInstruction::GetProviders { key, resp } => {
                self.handle_controller_get_providers(swarm, key, resp).await
            }
            SwarmInstruction::GetClosestPeers { key, resp } => {
                self.handle_controller_get_closest_peers(swarm, key, resp)
                    .await
            }
            SwarmInstruction::StartProviding { key, resp } => {
                self.handle_controller_start_providing(swarm, key, resp)
                    .await
            }
        }
    }

    async fn handle_controller_start_providing<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        resp: OneSender<OneReceiver<Res<()>>>,
    ) -> Res<()> {
        info!("getting key {:?}", key);
        let key = Key::new(&key);
        let (sender, receiver) = oneshot::channel::<Res<()>>();
        resp.send(receiver)?;

        let query_id = swarm.behaviour_mut().kademlia.start_providing(key)?;
        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::StartProviding { sender });
        Ok(())
    }

    async fn handle_controller_get_providers<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        resp: OneSender<OneReceiver<Res<HashSet<PeerId>>>>,
    ) -> Res<()> {
        info!("get providers {:?}", key);
        let key = Key::new(&key);
        let (sender, receiver) = oneshot::channel::<Res<HashSet<PeerId>>>();
        resp.send(receiver)?;

        let query_id = swarm.behaviour_mut().kademlia.get_providers(key);
        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::GetProviders { sender });
        Ok(())
    }

    async fn handle_controller_get_closest_peers<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        resp: OneSender<OneReceiver<Res<Vec<PeerId>>>>,
    ) -> Res<()> {
        info!("getting closest to: {:?}", key);
        let (sender, receiver) = oneshot::channel::<Res<Vec<PeerId>>>();
        resp.send(receiver)?;

        let query_id = swarm
            .behaviour_mut()
            .kademlia
            .get_closest_peers(key.as_bytes().to_vec());

        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::GetClosestPeers { sender });
        Ok(())
    }

    async fn handle_controller_put_record_to<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        value: Vec<u8>,
        remotes: Vec<PeerId>,
        resp: OneSender<OneReceiver<Res<()>>>,
    ) -> Res<()> {
        info!("getting key {:?}", key);
        let key = Key::new(&key);
        let (sender, receiver) = oneshot::channel::<Res<()>>();
        resp.send(receiver)?;
        let record = Record {
            key: key.clone(),
            value,
            publisher: None,
            expires: None,
        };

        let query_id =
            swarm
                .behaviour_mut()
                .kademlia
                .put_record_to(record, remotes.into_iter(), Quorum::One);

        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::PutTo { sender });
        Ok(())
    }

    async fn handle_controller_put<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        value: Vec<u8>,
        resp: OneSender<OneReceiver<Res<()>>>,
    ) -> Res<()> {
        info!("putting key {:?} val {:?}", key, value);
        let key = Key::new(&key);
        let record = Record {
            key: key.clone(),
            value,
            publisher: None,
            expires: None,
        };
        let (sender, receiver) = oneshot::channel::<Res<()>>();
        resp.send(receiver)?;
        // TODO this might have a race condition where the query is not yet in the map
        let query_id = swarm
            .behaviour_mut()
            .kademlia
            .put_record(record, Quorum::One)?;
        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::Put { sender });

        // let query_id = swarm.behaviour_mut().kademlia.start_providing(key)?;
        // let (sender, receiver) = oneshot::channel::<Res<()>>();
        // self.queries
        //     .lock()
        //     .await
        //     .insert(query_id, QueryResponse::Put { sender });

        Ok(())
    }

    async fn handle_controller_get<'t>(
        &self,
        swarm: &mut MutexGuard<'t, libp2p::Swarm<CombinedBehaviour>>,
        key: String,
        resp: OneSender<OneReceiver<Res<Bytes>>>,
    ) -> Res<()> {
        info!("getting key {:?}", key);
        let key = Key::new(&key);
        let (sender, receiver) = oneshot::channel::<Res<Bytes>>();
        resp.send(receiver)?;

        let query_id = swarm.behaviour_mut().kademlia.get_record(key);
        self.queries
            .lock()
            .await
            .insert(query_id, QueryResponse::Get { sender });
        Ok(())
    }
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "WireEvent")]
pub struct CombinedBehaviour {
    kademlia: Kademlia<MemoryStore>,
    // kademlia: Kademlia<LocalStore>,
    mdns: Behaviour,
}

impl From<KademliaEvent> for WireEvent {
    fn from(event: KademliaEvent) -> Self {
        WireEvent::Kademlia(event)
    }
}

impl From<mdns::Event> for WireEvent {
    fn from(event: mdns::Event) -> Self {
        WireEvent::Mdns(event)
    }
}

#[derive(Debug)]
pub enum WireEvent {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
}

fn generate_keypair() -> Keypair {
    Keypair::generate_ed25519()
}
