use crate::{settings::ISettings, storage::IStorage};
use async_std::io;
use async_trait::async_trait;
use base64::Engine as _;
use common::{ErrorKind, Res};
use futures::{AsyncBufReadExt, StreamExt};
use libp2p::{
    core::upgrade::Version,
    kad::{
        record::Key, store::MemoryStore, AddProviderOk, GetProvidersOk, GetRecordOk, Kademlia,
        KademliaConfig, KademliaEvent, PeerRecord, PutRecordOk, QueryResult, Quorum, Record,
    },
    mdns::{self, tokio::Behaviour},
    noise::NoiseAuthenticated,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::tokio::Transport,
    yamux::YamuxConfig,
    PeerId, Transport as _,
};
use libp2p_identity::Keypair;
use log::info;
use runtime_injector::{interface, Service, Svc};
use std::time::Duration;

interface! {
    dyn ISwarm = [
        Swarm,
    ]
}

#[async_trait]
pub trait ISwarm: Service {
    async fn start(&self) -> Res<()>;
}

pub struct Swarm(pub Svc<dyn ISettings>, pub Svc<dyn IStorage>);

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
enum WireEvent {
    Kademlia(KademliaEvent),
    Mdns(mdns::Event),
}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "WireEvent")]
struct CombinedBehaviour {
    kademlia: Kademlia<MemoryStore>,
    mdns: Behaviour,
}

#[async_trait]
impl ISwarm for Swarm {
    async fn start(&self) -> Res<()> {
        let local_key = Keypair::from_protobuf_encoding(
            &base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(&self.0.swarm().keypair)
                .map_err(|e| ErrorKind::KeypairBase64DecodeError(e))?,
        )
        .map_err(|e| ErrorKind::KeypairProtobufDecodeError(e))?;

        let local_peer_id = PeerId::from(local_key.public());
        info!("starting peer with id: {}", local_peer_id);
        // TODO make peer id persistent

        let mut swarm = {
            let cfg = KademliaConfig::default()
                .set_query_timeout(Duration::from_secs(60))
                .to_owned();
            let store = MemoryStore::new(local_peer_id);
            let mdns = Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();
            let kademlia = Kademlia::with_config(local_peer_id, store, cfg);
            let behaviour = CombinedBehaviour { kademlia, mdns };
            let transport = Transport::default()
                .upgrade(Version::V1)
                .authenticate(NoiseAuthenticated::xx(&local_key).unwrap())
                .multiplex(YamuxConfig::default())
                .boxed();
            SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build()
        };

        let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();

        swarm
            .listen_on(
                format!("/ip4/0.0.0.0/tcp/{}", self.0.swarm().port)
                    .parse()
                    .unwrap(),
            )
            .unwrap();

        loop {
            let line = stdin.next().await;
            handle_input_line(&mut swarm.behaviour_mut().kademlia, line.unwrap().unwrap());
            let event = swarm.select_next_some().await;
            println!("{:?}", event);
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening in {address:?}");
                }
                SwarmEvent::Behaviour(WireEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        swarm
                            .behaviour_mut()
                            .kademlia
                            .add_address(&peer_id, multiaddr);
                    }
                }
                SwarmEvent::Behaviour(WireEvent::Kademlia(
                    KademliaEvent::OutboundQueryProgressed { result, .. },
                )) => match result {
                    QueryResult::GetProviders(Ok(GetProvidersOk::FoundProviders {
                        key,
                        providers,
                        ..
                    })) => {
                        for peer in providers {
                            println!(
                                "Peer {peer:?} provides key {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap()
                            );
                        }
                    }
                    QueryResult::GetProviders(Err(err)) => {
                        eprintln!("Failed to get providers: {err:?}");
                    }
                    QueryResult::GetRecord(Ok(GetRecordOk::FoundRecord(PeerRecord {
                        record: Record { key, value, .. },
                        ..
                    }))) => {
                        println!(
                            "Got record {:?} {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap(),
                            std::str::from_utf8(&value).unwrap(),
                        );
                    }
                    QueryResult::GetRecord(Ok(_)) => {}
                    QueryResult::GetRecord(Err(err)) => {
                        eprintln!("Failed to get record: {err:?}");
                    }
                    QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
                        println!(
                            "Successfully put record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::PutRecord(Err(err)) => {
                        eprintln!("Failed to put record: {err:?}");
                    }
                    QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
                        println!(
                            "Successfully put provider record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::StartProviding(Err(err)) => {
                        eprintln!("Failed to put provider record: {err:?}");
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        // Ok(())
    }
}

fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String) {
    let mut args = line.split(' ');

    match args.next() {
        Some("GET") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            kademlia.get_record(key);
        }
        Some("GET_PROVIDERS") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            kademlia.get_providers(key);
        }
        Some("PUT") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            let value = {
                match args.next() {
                    Some(value) => value.as_bytes().to_vec(),
                    None => {
                        eprintln!("Expected value");
                        return;
                    }
                }
            };
            let record = Record {
                key,
                value,
                publisher: None,
                expires: None,
            };
            kademlia
                .put_record(record, Quorum::One)
                .expect("Failed to store record locally.");
        }
        Some("PUT_PROVIDER") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };

            kademlia
                .start_providing(key)
                .expect("Failed to start providing key");
        }
        _ => {
            eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
        }
    }
}
