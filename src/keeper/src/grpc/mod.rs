mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}
use crate::settings::{self, ISettings};
use crate::storage::IStorage;
use async_trait::async_trait;
use base64::Engine;
use common::consts::{GRPC_TIMEOUT, LOCALHOST};
use common::{ErrorKind, Res};
use keeper_grpc::keeper_grpc_server::KeeperGrpc;
use keeper_grpc::keeper_grpc_server::KeeperGrpcServer;
use keeper_grpc::{GetRequest, GetResponse, PutRequest, PutResponse};
use libp2p_identity::Keypair;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, Svc, TypedProvider,
};
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

interface! {
    dyn IGrpcHandler = [
        GrpcHandler,
    ]
}

pub struct GrpcProvider;
impl TypedProvider for GrpcProvider {
    type Result = GrpcHandler;

    fn provide_typed(
        &mut self,
        _injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Svc<Self::Result>> {
        let port = _injector.get::<Svc<dyn ISettings>>().unwrap().grpc().port;
        let storage: Svc<dyn IStorage> = _injector.get().unwrap();

        Ok(Svc::new(GrpcHandler {
            inner: Inner { storage },
            port,
        }))
    }
}

#[async_trait]
pub trait IGrpcHandler: Service {
    async fn start(&self) -> Res<()>;
}

#[derive(Clone)]
struct Inner {
    storage: Svc<dyn IStorage>,
}

pub struct GrpcHandler {
    inner: Inner,
    port: u16,
}

#[async_trait]
impl IGrpcHandler for GrpcHandler {
    async fn start(&self) -> Res<()> {
        // self.inner.
        let addr = format!("{}:{}", LOCALHOST, self.port)
            .parse::<SocketAddr>()
            .map_err(|e| ErrorKind::SettingsParseError(e.to_string()))?;

        info!("grpc listening on {}", addr);

        let middleware = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(GRPC_TIMEOUT))
            .layer(tonic::service::interceptor(|req| Ok(req)))
            .into_inner();

        Server::builder()
            .layer(middleware)
            .add_service(KeeperGrpcServer::new(self.inner.clone()))
            .serve(addr)
            .await
            .map_err(|e| ErrorKind::GrpcServerStartFailed(e))?;

        Ok(())
    }
}

#[async_trait]
impl KeeperGrpc for Inner {
    async fn put(
        &self,
        request: Request<PutRequest>,
    ) -> std::result::Result<Response<PutResponse>, Status> {
        let request = request.into_inner();
        info!("received a put request for {}", request.path);
        self.storage
            .put(request.path.into(), request.content)
            .await
            .map_err(|e| match e.kind() {
                ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
                _ => Status::unknown("Unknown storage error".to_string()),
            })?;

        let reply = PutResponse {};

        Ok(Response::new(reply))
    }

    async fn get(
        &self,
        request: Request<GetRequest>, // Accept request of type HelloRequest
    ) -> std::result::Result<Response<GetResponse>, Status> {
        let request = request.into_inner();
        info!("received a get request for {}", request.path);
        let content = self
            .storage
            .get(request.path.into())
            .await
            .map_err(|e| match e.kind() {
                ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
                _ => Status::unknown("Unknown storage error".to_string()),
            })?;

        let reply = GetResponse { content: content };

        Ok(Response::new(reply))
    }
}

impl GrpcHandler {
    async fn generate_keypair(&self) -> String {
        let local_key = Keypair::generate_ed25519();
        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD
            .encode(local_key.to_protobuf_encoding().unwrap());
        return encoded;
    }
}
