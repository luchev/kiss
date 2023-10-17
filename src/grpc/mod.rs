use crate::ledger::{ILedger, ImmuLedger};
use crate::p2p::controller::ISwarmController;
use crate::settings::ISettings;
use crate::storage::IStorage;
use crate::util::consts::{GRPC_TIMEOUT, LOCALHOST};
use crate::util::grpc::kiss_grpc::kiss_service_server::KissService;
use crate::util::grpc::kiss_grpc::kiss_service_server::KissServiceServer;
use crate::util::grpc::kiss_grpc::{
    GetProvidersRequest, GetProvidersResponse, RetrieveRequest, RetrieveResponse, StoreRequest,
    StoreResponse, VerifyRequest, VerifyResponse,
};
use crate::util::hasher::{self, hash};
use crate::util::{ErrorKind, Res};
use async_trait::async_trait;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

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
        let storage = injector.get::<Svc<dyn IStorage>>()?;
        let swarm_controller = injector.get::<Svc<dyn ISwarmController>>()?;

        Ok(GrpcHandler {
            inner: Inner {
                storage,
                swarm_controller,
                ledger: injector.get::<Svc<Mutex<ImmuLedger>>>()?,
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
    storage: Svc<dyn IStorage>,
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

        let listener = TcpListener::bind(addr).await?;
        let real_addr = listener.local_addr()?;

        info!("grpc listening on {}", real_addr);

        let middleware = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(GRPC_TIMEOUT))
            .layer(tonic::service::interceptor(Ok))
            .into_inner();

        Server::builder()
            .layer(middleware)
            .add_service(KissServiceServer::new(self.inner.clone()))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await?;

        Ok(())
    }
}

#[async_trait]
impl KissService for Inner {
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
        info!("received a store request for {}", request.name);
        let file_hash = hash(&request.content);
        info!("{}", file_hash);
        let mut ledger = self.ledger.lock().await;
        let file_uuid = ledger
            .create_contract(file_hash, request.ttl)
            .await
            .map_err(|e| Status::unknown(e.to_string()))?;

        info!("created contract for {}", file_uuid);

        let res = self
            .swarm_controller
            .set(file_uuid.clone(), request.content)
            .await;
        info!("kad result {:?}", res);
        // self.storage
        //     .put(request.path.into(), request.content)
        //     .await
        //     .map_err(|e| match e.kind() {
        //         ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
        //         _ => Status::unknown("Unknown storage error".to_string()),
        //     })?;

        match res {
            Ok(_) => {
                info!("stored file {}", file_uuid);
                Ok(Response::new(StoreResponse { name: file_uuid }))
            }
            Err(e) => {
                info!("failed to store file {}", file_uuid);
                Err(Status::internal(e.to_string()))
            }
        }
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
        info!("kad result {:?}", res);
        let content =
            res.map_err(|e| Status::not_found(format!("failed getting from swarm: {}", e)))?;

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
        info!("received a get providers request for {}", request.name);
        self.swarm_controller
            .get_providers(request.name.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))
            .map(|providers| {
                Ok(Response::new(GetProvidersResponse {
                    name: request.name,
                    providers: providers.into_iter().map(|x| x.to_string()).collect(),
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
            .map_err(|e| Status::not_found(format!("failed to get file from swarm: {}", e)))?;

        let reply = VerifyResponse {
            hash: hash(&content),
        };
        Ok(Response::new(reply))
    }
}
