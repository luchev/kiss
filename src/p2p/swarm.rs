use crate::bench::Bench;
use crate::p2p::peer_id::keypair_with_leading_zeros;
use crate::p2p::store::LocalStoreConfig;
// use crate::p2p::memorystore::{MemoryStore, MemoryStoreConfig};
use crate::settings::ISettings;
use crate::storage::IStorage;
use crate::util::consts;
use crate::util::{
    types::{Bytes, CommandToSwarm, OneReceiver, OneSender},
    Er, ErrorKind, Res,
};
use crate::verifier::por::{VerificationServer, VerificationServerConfig};
use async_trait::async_trait;
use base64::Engine as _;
use futures::StreamExt;
use libp2p::request_response::{
    InboundFailure, OutboundFailure, ProtocolSupport, RequestId, ResponseChannel,
};
use libp2p::StreamProtocol;
use libp2p::{
    core::upgrade::Version,
    kad::{
        record::Key, GetRecordOk, GetRecordResult, Kademlia, KademliaConfig, KademliaEvent,
        PeerRecord, PutRecordResult, QueryId, QueryResult, Quorum, Record,
    },
    mdns::{self, tokio::Behaviour},
    noise, request_response,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::tokio::Transport,
    yamux, PeerId, Transport as _,
};
use libp2p_identity::Keypair;
use libp2p_kad::{
    AddProviderOk, AddProviderResult, GetClosestPeersOk, GetClosestPeersResult, GetProvidersOk,
    GetProvidersResult, Mode,
};
use log::{debug, info, warn};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    time::Duration,
};
use tokio::{
    select,
    sync::{mpsc::Receiver, oneshot, Mutex, MutexGuard},
};
use uuid::Uuid;

use super::store::LocalStore;

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
        let commands_from_controller: Svc<Mutex<Receiver<CommandToSwarm>>> = injector.get()?;
        let storage: Svc<dyn IStorage> = injector.get()?;
        let bench = injector.get::<Svc<Mutex<Bench>>>()?;

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
            None => keypair_with_leading_zeros(consts::DEFAULT_LEADING_ZEROS),
        };

        let local_peer_id = PeerId::from(local_key.public());
        info!("starting peer with id: {}", local_peer_id);

        let mut swarm = {
            let cfg = KademliaConfig::default()
                .set_query_timeout(Duration::from_secs(60))
                .set_max_packet_size(1024 * 1024 * 1024)
                .to_owned();
            // let store = MemoryStore::with_config(
            //     local_peer_id,
            //     MemoryStoreConfig {
            //         max_records: 150000,
            //         max_value_bytes: 1024 * 1024 * 200,
            //         max_provided_keys: 150000,
            //         max_providers_per_key: 5,
            //     },
            // );

            let store = LocalStore::with_config(
                local_peer_id,
                LocalStoreConfig {
                    max_records: 150000,
                    max_value_bytes: 1024 * 1024 * 1024,
                    max_provided_keys: 150000,
                    max_providers_per_key: 20,
                },
                storage.clone(),
            );
            let mdns = Behaviour::new(mdns::Config::default(), local_peer_id).map_err(|e| {
                InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<Swarm>(),
                    inner: Box::<Er>::new(ErrorKind::BehaviourInitFailed(e).into()),
                }
            })?;

            let mut kademlia = Kademlia::with_config(local_peer_id, store, cfg);
            kademlia.set_mode(Some(Mode::Server));
            let req_res = request_response::cbor::Behaviour::new(
                [(
                    StreamProtocol::new("/verification/1.0.0"),
                    ProtocolSupport::Full,
                )],
                request_response::Config::default(),
            );
            let behaviour = CombinedBehaviour {
                kademlia,
                mdns,
                req_res,
            };
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
                inner: Box::<Er>::new(
                    ErrorKind::SwarmListenFailed(e, settings.swarm().port).into(),
                ),
            })?;

        Ok(Swarm {
            local_peer_id,
            storage,
            inner: Mutex::new(swarm),
            commands_from_controller,
            bench,
            queries: Mutex::new(HashMap::new()),
            requests: Mutex::new(HashMap::new()),
        })
    }
}

#[async_trait]
pub trait ISwarm: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Swarm {
    local_peer_id: PeerId,
    storage: Svc<dyn IStorage>,
    inner: Mutex<libp2p::Swarm<CombinedBehaviour>>,
    commands_from_controller: Svc<Mutex<Receiver<CommandToSwarm>>>,
    bench: Svc<Mutex<Bench>>,
    queries: Mutex<HashMap<QueryId, QueryResponse>>,
    requests: Mutex<HashMap<RequestId, QueryResponse>>,
}

#[derive(Debug)]
pub struct QueryGetResponse {
    pub file: Bytes,
    pub origin_peer_id: PeerId,
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
        sender: OneSender<Res<QueryGetResponse>>,
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
    VerificationRequest {
        sender: OneSender<Res<VerificationResponse>>,
    },
}

#[async_trait]
impl ISwarm for Swarm {
    async fn start(&self) -> Res<()> {
        let mut swarm = self.inner.lock().await;
        let mut receiver = self.commands_from_controller.lock().await;
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
        // debug!("swarm event {:?}", event);
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("listening on {address:?}");
            }
            SwarmEvent::Behaviour(WireEvent::Mdns(mdns::Event::Discovered(list))) => {
                debug!("discovered peers: {list:?}");
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
                    debug!("routing updated",);
                }
                KademliaEvent::InboundRequest { request, .. } => {
                    debug!("inbound request: {:?}", request);
                }
                _ => warn!("unhandled kademlia event: {:?}", event),
            },
            SwarmEvent::Behaviour(WireEvent::ReqRes(event)) => match event {
                request_response::Event::Message { peer: _, message } => match message {
                    request_response::Message::Request {
                        request_id: _, // allows async processing
                        request,
                        channel,
                    } => {
                        self.handle_reqres_message_request(swarm, request, channel)
                            .await?
                    }
                    request_response::Message::Response {
                        request_id,
                        response,
                    } => {
                        self.handle_reqres_message_response(response, request_id)
                            .await?
                    }
                },
                request_response::Event::ResponseSent { peer, request_id } => {
                    self.handle_response_sent(peer, request_id).await?
                }
                request_response::Event::InboundFailure {
                    peer,
                    request_id,
                    error,
                } => self.handle_inbound_failure(peer, request_id, error).await?,
                request_response::Event::OutboundFailure {
                    peer,
                    request_id,
                    error,
                } => {
                    self.handle_outbound_failure(peer, request_id, error)
                        .await?
                }
            },
            _ => {}
        }
        Ok(())
    }

    async fn handle_reqres_message_request(
        &self,
        swarm: &mut MutexGuard<'_, libp2p::Swarm<CombinedBehaviour>>,
        request: VerificationRequest,
        channel: ResponseChannel<VerificationResponse>,
    ) -> Res<()> {
        match self.storage.get(request.file_name.clone().into()).await {
            Ok(file) => {
                let server_config = VerificationServerConfig::from_file(file.value);
                let server = VerificationServer::new(server_config);
                let response = server.fulfill_challenge(request.challenge_vector);
                swarm
                    .behaviour_mut()
                    .req_res
                    .send_response(
                        channel,
                        VerificationResponse {
                            file_name: request.file_name,
                            response_vector: response,
                        },
                    )
                    .map_err(|_| ErrorKind::SwarmReqResSendResponseError.into())
            }
            Err(_) => {
                if let Some(deleted_at) = self
                    .bench
                    .lock()
                    .await
                    .deleted_files
                    .remove(&request.file_name)
                {
                    let elapsed = deleted_at.elapsed().unwrap();
                    info!(
                        "file {} was discovered corrupted after {}",
                        request.file_name,
                        elapsed.as_millis()
                    );
                }
                swarm
                    .behaviour_mut()
                    .req_res
                    .send_response(
                        channel,
                        VerificationResponse {
                            file_name: request.file_name,
                            response_vector: Vec::new(),
                        },
                    )
                    .map_err(|_| ErrorKind::SwarmReqResSendResponseError.into())
            }
        }
        // let file = self.storage.get(request.file_name.clone().into()).await?;
        // let server_config = VerificationServerConfig::from_file(file.value);
        // let server = VerificationServer::new(server_config);
        // let response = server.fulfill_challenge(request.challenge_vector);
        // swarm
        //     .behaviour_mut()
        //     .req_res
        //     .send_response(
        //         channel,
        //         VerificationResponse {
        //             file_name: request.file_name,
        //             response_vector: response,
        //         },
        //     )
        //     .map_err(|_| ErrorKind::SwarmReqResSendResponseError.into())
    }

    async fn handle_reqres_message_response(
        &self,
        response: VerificationResponse,
        request_id: RequestId,
    ) -> Res<()> {
        let response_channel = match self.requests.lock().await.remove(&request_id) {
            Some(QueryResponse::VerificationRequest { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannelForRequest(request_id))?,
        };
        response_channel
            .send(Ok(response))
            .map_err(|_| ErrorKind::SwarmReqResSendResponseError)?;
        Ok(())
    }

    async fn handle_response_sent(&self, _peer: PeerId, request_id: RequestId) -> Res<()> {
        debug!("response sent for request_id: {:?}", request_id);
        Ok(())
    }

    async fn handle_inbound_failure(
        &self,
        _peer: PeerId,
        request_id: RequestId,
        error: InboundFailure,
    ) -> Res<()> {
        let response_channel = match self.requests.lock().await.remove(&request_id) {
            Some(QueryResponse::VerificationRequest { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannelForRequest(request_id))?,
        };
        response_channel
            .send(Err(ErrorKind::RequestInboundFailure(error).into()))
            .map_err(|_| ErrorKind::SwarmReqResSendResponseError)?;
        Ok(())
    }

    async fn handle_outbound_failure(
        &self,
        _peer: PeerId,
        request_id: RequestId,
        error: OutboundFailure,
    ) -> Res<()> {
        let response_channel = match self.requests.lock().await.remove(&request_id) {
            Some(QueryResponse::VerificationRequest { sender }) => sender,
            _ => Err(ErrorKind::InvalidResponseChannelForRequest(request_id))?,
        };
        response_channel
            .send(Err(ErrorKind::RequestOutboundFailure(error).into()))
            .map_err(|_| ErrorKind::SwarmReqResSendResponseError)?;
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
            _ => {
                info!("channel already closed for query: {:?}", id);
                return Ok(());
                // Err(ErrorKind::InvalidResponseChannel(id))?
            }
        };

        match message {
            Ok(GetRecordOk::FoundRecord(PeerRecord {
                record: Record { value, .. },
                peer,
            })) => {
                let peer = match peer {
                    Some(peer) => peer,
                    None => self.local_peer_id,
                };
                info!("found record for peer: {:?}", peer);
                response_channel.send(Ok(QueryGetResponse {
                    file: value,
                    origin_peer_id: peer,
                }))?
            }
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
        instruction: Option<CommandToSwarm>,
        swarm: &mut MutexGuard<'a, libp2p::Swarm<CombinedBehaviour>>,
    ) -> Res<()> {
        let instruction = instruction.ok_or(ErrorKind::MissingInstruction)?;
        debug!("instruction {}", instruction);
        match instruction {
            CommandToSwarm::PutLocal { key, value, resp } => {
                self.handle_controller_put(swarm, key, value, resp).await
            }
            CommandToSwarm::PutRemote {
                key,
                value,
                remotes,
                resp,
            } => {
                self.handle_controller_put_record_to(swarm, key, value, remotes, resp)
                    .await
            }
            CommandToSwarm::Get { key, resp } => self.handle_controller_get(swarm, key, resp).await,
            CommandToSwarm::GetProviders { key, resp } => {
                self.handle_controller_get_providers(swarm, key, resp).await
            }
            CommandToSwarm::GetClosestPeers { key, resp } => {
                self.handle_controller_get_closest_peers(swarm, key, resp)
                    .await
            }
            CommandToSwarm::StartProviding { key, resp } => {
                self.handle_controller_start_providing(swarm, key, resp)
                    .await
            }
            CommandToSwarm::RequestVerification {
                peer,
                file_uuid,
                challenge_vector,
                resp,
            } => {
                self.handle_controller_request_verification(
                    swarm,
                    peer,
                    file_uuid,
                    challenge_vector,
                    resp,
                )
                .await
            }
        }
    }

    async fn handle_controller_request_verification(
        &self,
        swarm: &mut MutexGuard<'_, libp2p::Swarm<CombinedBehaviour>>,
        peer: PeerId,
        file_name: String,
        challenge_vector: Vec<u64>,
        resp: OneSender<OneReceiver<Res<VerificationResponse>>>,
    ) -> Res<()> {
        let (sender, receiver) = oneshot::channel::<Res<VerificationResponse>>();
        resp.send(receiver)?;

        let request_id = swarm.behaviour_mut().req_res.send_request(
            &peer,
            VerificationRequest {
                file_name,
                challenge_vector,
            },
        );

        self.requests
            .lock()
            .await
            .insert(request_id, QueryResponse::VerificationRequest { sender });
        Ok(())
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
        key: Uuid,
        resp: OneSender<OneReceiver<Res<Vec<PeerId>>>>,
    ) -> Res<()> {
        debug!("getting closest to: {:?}", key);
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
        resp: OneSender<OneReceiver<Res<QueryGetResponse>>>,
    ) -> Res<()> {
        debug!("kad get key {:?}", key);
        let key = Key::new(&key);
        let (sender, receiver) = oneshot::channel::<Res<QueryGetResponse>>();
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
    // kademlia: Kademlia<MemoryStore>,
    kademlia: Kademlia<LocalStore>,
    mdns: Behaviour,
    req_res: request_response::cbor::Behaviour<VerificationRequest, VerificationResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationRequest {
    pub file_name: String,
    pub challenge_vector: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResponse {
    pub file_name: String,
    pub response_vector: Vec<u64>,
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

impl From<request_response::Event<VerificationRequest, VerificationResponse>> for WireEvent {
    fn from(event: request_response::Event<VerificationRequest, VerificationResponse>) -> Self {
        WireEvent::ReqRes(event)
    }
}

#[derive(Debug)]
pub enum WireEvent {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
    ReqRes(request_response::Event<VerificationRequest, VerificationResponse>),
}
