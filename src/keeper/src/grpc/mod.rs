mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}
use crate::settings::Settings;
use crate::storage::Storage;
use async_trait::async_trait;
use common::consts::{GRPC_TIMEOUT, LOCALHOST};
use common::errors::{ErrorKind, Result};
use keeper_grpc::keeper_grpc_server::KeeperGrpc;
use keeper_grpc::keeper_grpc_server::KeeperGrpcServer;
use keeper_grpc::{GetRequest, GetResponse, PutRequest, PutResponse};
use log::info;
use runtime_injector::{interface, Service, Svc};
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

#[async_trait]
pub trait GrpcProvider: Service {
    async fn start(&self) -> Result<()>;
}

pub struct GrpcProviderImpl(pub Svc<dyn Settings>, pub Svc<dyn Storage>);

struct Grpc {
    storage: Svc<dyn Storage>,
}

#[async_trait]
impl GrpcProvider for GrpcProviderImpl {
    async fn start(&self) -> Result<()> {
        let grpc = Grpc {
            storage: self.1.clone(),
        };
        let addr = format!("{}:{}", LOCALHOST, self.0.grpc().port)
            .parse::<SocketAddr>()
            .map_err(|e| ErrorKind::SettingsParseError(e.to_string()))?;

        info!("grpc listening on {}", addr);

        let middleware = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(GRPC_TIMEOUT))
            .layer(tonic::service::interceptor(|req| Ok(req)))
            .into_inner();

        Server::builder()
            .layer(middleware)
            .add_service(KeeperGrpcServer::new(grpc))
            .serve(addr)
            .await
            .map_err(|e| ErrorKind::GrpcServerStartFailed(e))?;

        Ok(())
    }
}

#[async_trait]
impl KeeperGrpc for Grpc {
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

interface! {
    dyn GrpcProvider = [
        GrpcProviderImpl,
    ]
}
