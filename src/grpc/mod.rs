use crate::ledger::{ILedger, ImmuLedger};
use crate::p2p::controller::ISwarmController;
use crate::settings::ISettings;
use crate::util::consts::{self, GRPC_TIMEOUT, LOCALHOST};
use crate::util::debug::print_now;
use crate::util::grpc::kiss_grpc::kiss_service_server::KissService;
use crate::util::grpc::kiss_grpc::kiss_service_server::KissServiceServer;
use crate::util::grpc::kiss_grpc::{
    GetProvidersRequest, GetProvidersResponse, RetrieveRequest, RetrieveResponse, StoreRequest,
    StoreResponse, VerifyRequest, VerifyResponse, *,
};
use crate::util::hasher::{self, hash};
use crate::util::{ErrorKind, Res};
use crate::verifier::por::{VerificationClient, VerificationClientConfig};
use async_trait::async_trait;
use libp2p_identity::PeerId;
use log::{debug, info};
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use uuid::Uuid;

interface! {
    dyn IGrpcHandler = [
        GrpcHandler,
    ]
}

pub struct GrpcProvider;
impl ServiceFactory<()> for GrpcProvider {
    type Result = GrpcHandler;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let port = injector.get::<Svc<dyn ISettings>>()?.grpc().port;
        let swarm_controller = injector.get::<Svc<dyn ISwarmController>>()?;
        let ledger = injector.get::<Svc<Mutex<ImmuLedger>>>()?;

        Ok(GrpcHandler {
            inner: Inner {
                swarm_controller,
                ledger,
            },
            port,
        })
    }
}

#[async_trait]
pub trait IGrpcHandler: Service {
    async fn start(&self) -> Res<()>;
}

#[derive(Clone)]
struct Inner {
    swarm_controller: Svc<dyn ISwarmController>,
    ledger: Svc<Mutex<ImmuLedger>>,
}

pub struct GrpcHandler {
    inner: Inner,
    port: u16,
}

#[async_trait]
impl IGrpcHandler for GrpcHandler {
    async fn start(&self) -> Res<()> {
        let addr = format!("{}:{}", LOCALHOST, self.port)
            .parse::<SocketAddr>()
            .map_err(|e| ErrorKind::SettingsParseError(e.to_string()))?;

        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ErrorKind::IoDetailed(e, self.port))?;
        let real_addr = listener
            .local_addr()
            .map_err(|e| ErrorKind::IoDetailed(e, self.port))?;

        info!("grpc listening on {}", real_addr);

        let middleware = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(GRPC_TIMEOUT))
            .layer(tonic::service::interceptor(Ok))
            .into_inner();

        Server::builder()
            .layer(middleware)
            .add_service(
                KissServiceServer::new(self.inner.clone())
                    .max_decoding_message_size(1024 * 1024 * 1024)
                    .max_encoding_message_size(1024 * 1024 * 1024),
            )
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl KissService for Inner {
    async fn verify_file(
        &self,
        request: Request<VerifyFileRequest>,
    ) -> Result<Response<VerifyFileResponse>, Status> {
        let request = request.into_inner();

        let contracts = self
            .ledger
            .lock()
            .await
            .get_contracts(request.file_uuid.clone())
            .await
            .map_err(|e| Status::unknown(e.to_string()))?;

        let mut result = vec![];
        for contract in contracts.iter() {
            let verification_client =
                VerificationClient::new(VerificationClientConfig::from_contract(&contract));
            let challenge = verification_client.make_challenge_vector();
            let response = self
                .swarm_controller
                .request_verification(
                    contract.peer_id,
                    request.file_uuid.clone(),
                    challenge.clone(),
                )
                .await;
            match response {
                Ok(response) => result.push(VerificationForPeer {
                    peer_id: contract.peer_id.to_string(),
                    verified: verification_client.audit(challenge, response),
                }),
                Err(_) => result.push(VerificationForPeer {
                    peer_id: contract.peer_id.to_string(),
                    verified: false,
                }),
            }
        }
        Ok(Response::new(VerifyFileResponse {
            verifications: result,
        }))
    }

    // async fn put(
    //     &self,
    //     request: Request<PutRequest>,
    // ) -> std::result::Result<Response<PutResponse>, Status> {
    //     let request = request.into_inner();
    //     info!("received a put request for {}", request.path);
    //     let res = self
    //         .swarm_controller
    //         .set(request.path, request.content)
    //         .await;
    //     info!("kad result {:?}", res);
    //     // self.storage
    //     //     .put(request.path.into(), request.content)
    //     //     .await
    //     //     .map_err(|e| match e.kind() {
    //     //         ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
    //     //         _ => Status::unknown("Unknown storage error".to_string()),
    //     //     })?;

    //     let reply = PutResponse {};
    //     Ok(Response::new(reply))
    // }

    // async fn get(
    //     &self,
    //     request: Request<GetRequest>,
    // ) -> std::result::Result<Response<GetResponse>, Status> {
    //     let request = request.into_inner();
    //     info!("received a get request for {}", request.path);
    //     let res = self.swarm_controller.get(request.path).await;
    //     info!("kad result {:?}", res);
    //     let content =
    //         res.map_err(|e| Status::not_found(format!("failed getting from swarm: {}", e)))?;
    //     // let content = self
    //     //     .storage
    //     //     .get(request.path.into())
    //     //     .await
    //     //     .map_err(|e| match e.kind() {
    //     //         ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
    //     //         _ => Status::unknown("Unknown storage error".to_string()),
    //     //     })?;

    //     let reply = GetResponse { content };
    //     Ok(Response::new(reply))
    // }

    async fn store(
        &self,
        request: Request<StoreRequest>,
    ) -> std::result::Result<Response<StoreResponse>, Status> {
        let request = request.into_inner();
        let start_time = SystemTime::now();
        debug!("store request for {}", request.name);

        let file_hash = hash(&request.content);
        debug!("{}", file_hash);

        let file_uuid = Uuid::new_v4();

        let closest = self.swarm_controller.get_closest_peers(file_uuid).await;
        debug!("closest peers: {:?}", closest);
        let closest_peers =
            closest.map_err(|e| Status::internal(format!("no closest peers {}", e.to_string())))?;

        let result = self
            .swarm_controller
            .put_to(
                file_uuid.clone().to_string(),
                request.content.clone(),
                closest_peers
                    .iter()
                    .take(consts::REPLICATION_FACTOR)
                    .cloned()
                    .collect(),
            )
            .await;

        debug!("put finished: {:?}", result);

        if let Err(e) = result {
            info!("failed to store file {}, {:?}", file_uuid, e);
            return Err(Status::internal(e.to_string()));
        }

        for peer in closest_peers.iter().take(consts::REPLICATION_FACTOR) {
            let client_config = VerificationClientConfig::from_file(&request.content);
            let (secret_n, secret_m, rows, cols) = client_config.to_contract();
            // retry writing the contract tot he ledger 10 times:
            let mut success = false;
            let mut last_error = String::new();
            for _ in 0..10 {
                let res = self
                    .ledger
                    .lock()
                    .await
                    .create_contract(
                        *peer,
                        file_uuid,
                        file_hash.clone(),
                        request.ttl,
                        secret_n.clone(),
                        secret_m.clone(),
                        rows,
                        cols,
                    )
                    .await;
                if let Err(e) = res {
                    last_error = e.to_string();
                } else {
                    success = true;
                    break;
                }
            }
            if !success {
                return Err(Status::internal(format!(
                    "failed to write contract to immudb: {}",
                    last_error
                )));
            }
        }

        let since_start = start_time.elapsed().unwrap().as_millis();
        info!(
            "stored file {} at peers [{}] for {} ms",
            file_uuid,
            closest_peers
                .iter()
                .take(consts::REPLICATION_FACTOR)
                .cloned()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            since_start
        );

        Ok(Response::new(StoreResponse {
            name: file_uuid.to_string(),
        }))
    }

    async fn retrieve(
        &self,
        request: Request<RetrieveRequest>,
    ) -> std::result::Result<Response<RetrieveResponse>, Status> {
        let request = request.into_inner();
        info!("received a get request for {}", request.name);
        let mut ledger = self.ledger.lock().await;
        let contract = ledger
            .get_contract(request.name.clone())
            .await
            .map_err(|e| Status::unknown(e.to_string()))?;

        let res = self.swarm_controller.get(request.name.clone()).await;
        info!("get finished: {:?}", res);
        let content = res
            .map_err(|e| Status::not_found(format!("failed getting from swarm: {}", e)))?
            .file;

        let file_hash = hasher::hash(&content);
        if file_hash != contract.file_hash {
            return Err(Status::data_loss("file has been modified"));
        }

        Ok(Response::new(RetrieveResponse {
            name: request.name,
            content,
        }))
    }

    async fn get_providers(
        &self,
        request: Request<GetProvidersRequest>,
    ) -> std::result::Result<Response<GetProvidersResponse>, Status> {
        let request = request.into_inner();
        info!("get providers request: {}", request.name);
        self.swarm_controller
            .get_providers(request.name.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))
            .map(|providers| {
                info!("get providers result: {:?}", providers);
                Ok(Response::new(GetProvidersResponse {
                    name: request.name,
                    providers: providers.into_iter().map(|x| x.to_string()).collect(),
                }))
            })?
    }

    async fn get_closest_peers(
        &self,
        request: Request<GetClosestPeersRequest>,
    ) -> std::result::Result<Response<GetClosestPeersResponse>, Status> {
        let request = request.into_inner();
        debug!("get closest peers request for {}", request.uuid);
        let file_uuid = Uuid::from_str(request.uuid.as_str())
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.swarm_controller
            .get_closest_peers(file_uuid)
            .await
            .map_err(|e| Status::internal(e.to_string()))
            .map(|peers| {
                debug!("get closest peers result: {:?}", peers);
                Ok(Response::new(GetClosestPeersResponse {
                    uuid: request.uuid,
                    peer_uuids: peers.into_iter().map(|x| x.to_string()).collect(),
                }))
            })?
    }

    async fn verify(
        &self,
        request: Request<VerifyRequest>,
    ) -> std::result::Result<Response<VerifyResponse>, Status> {
        let request = request.into_inner();
        info!("received a verify request for {}", request.path);
        let content = self
            .swarm_controller
            .get(request.path)
            .await
            .map_err(|e| Status::not_found(format!("failed to get file from swarm: {}", e)))?
            .file;

        let reply = VerifyResponse {
            hash: hash(&content),
        };
        Ok(Response::new(reply))
    }

    async fn start_providing(
        &self,
        request: Request<StartProvidingRequest>,
    ) -> std::result::Result<Response<StartProvidingResponse>, Status> {
        let request = request.into_inner();
        info!("received a get providers request for {}", request.uuid);
        self.swarm_controller
            .start_providing(request.uuid.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))
            .map(|()| Ok(Response::new(StartProvidingResponse { uuid: request.uuid })))?
    }

    async fn put_to(
        &self,
        request: Request<PutToRequest>,
    ) -> std::result::Result<Response<PutToResponse>, Status> {
        let request = request.into_inner();
        let file_hash = hash(&request.content);
        let mut peers = Vec::new();
        for peer_uuid in request.peer_uuids.iter() {
            peers.push(
                PeerId::from_str(peer_uuid).map_err(|e| Status::invalid_argument(e.to_string()))?,
            );
        }
        let mut ledger = self.ledger.lock().await;
        let file_uuid = Uuid::new_v4();
        let client_config = VerificationClientConfig::from_file(&request.content);
        let (secret_n, secret_m, rows, cols) = client_config.to_contract();
        for peer in peers.iter() {
            ledger
                .create_contract(
                    peer.clone(),
                    file_uuid,
                    file_hash.clone(),
                    request.ttl,
                    secret_n.clone(),
                    secret_m.clone(),
                    rows,
                    cols,
                )
                .await
                .map_err(|e| Status::unknown(e.to_string()))?;
        }

        info!("created contract for {}", file_uuid);

        let peer_uuids: Result<Vec<_>, Status> = request
            .peer_uuids
            .iter()
            .map(|x| PeerId::from_str(x).map_err(|e| Status::invalid_argument(e.to_string())))
            .collect();

        let res = self
            .swarm_controller
            .put_to(file_uuid.clone().to_string(), request.content, peer_uuids?)
            .await;
        info!("put to finished: {:?}", res);

        match res {
            Ok(_) => {
                info!("stored file {}", file_uuid);
                Ok(Response::new(PutToResponse {
                    uuid: file_uuid.to_string(),
                }))
            }
            Err(e) => {
                info!("failed to store file {}", file_uuid);
                Err(Status::internal(e.to_string()))
            }
        }
    }
}
