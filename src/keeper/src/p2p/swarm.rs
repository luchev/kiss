use crate::settings::ISettings;
use async_trait::async_trait;
use base64::Engine as _;
use common::{
    types::{Bytes, OneReceiver, OneSender, SwarmInstruction},
    Er, ErrorKind, Res,
};
use futures::StreamExt;
use libp2p::{
    core::upgrade::Version,
    kad::{
        record::Key,
        store::{MemoryStore, MemoryStoreConfig},
        GetRecordOk, GetRecordResult, Kademlia, KademliaConfig, KademliaEvent, PeerRecord,
        PutRecordResult, QueryId, QueryResult, Quorum, Record,
    },
    mdns::{self, tokio::Behaviour},
    noise,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::tokio::Transport,
    yamux, PeerId, Transport as _,
};
use libp2p_identity::Keypair;
use log::{info, warn};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::{collections::HashMap, fmt::Debug, time::Duration};
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
                    max_providers_per_key: 20,
                },
            );
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

enum QueryResponse {
    Put { sender: OneSender<Res<()>> },
    Get { sender: OneSender<Res<Bytes>> },
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
            SwarmEvent::Behaviour(WireEvent::Kademlia(
                KademliaEvent::OutboundQueryProgressed { result, id, .. },
            )) => match result {
                QueryResult::GetRecord(message) => self.handle_get_record(message, id).await?,
                QueryResult::PutRecord(result) => self.handle_put_record(result, id).await?,
                _ => warn!("unhandled query result: {:?}", result),
            },
            _ => {}
        }
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
            SwarmInstruction::Put { key, value, resp } => {
                self.handle_controller_put(swarm, key, value, resp).await
            }
            SwarmInstruction::Get { key, resp } => {
                self.handle_controller_get(swarm, key, resp).await
            }
        }
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
            key,
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
#[behaviour(out_event = "WireEvent")]
pub struct CombinedBehaviour {
    kademlia: Kademlia<MemoryStore>,
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
