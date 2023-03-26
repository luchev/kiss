mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}
use crate::settings::ISettings;
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
use runtime_injector::{interface, Service, Svc};
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

interface! {
    dyn IGrpcProvider = [
        GrpcProvider,
    ]
}

#[async_trait]
pub trait IGrpcProvider: Service {
    async fn start(&self) -> Res<()>;
}

pub struct GrpcProvider(pub Svc<dyn ISettings>, pub Svc<dyn IStorage>);

struct Grpc {
    storage: Svc<dyn IStorage>,
}

#[async_trait]
impl IGrpcProvider for GrpcProvider {
    async fn start(&self) -> Res<()> {
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

impl Grpc {
    async fn generate_keypair(&self) -> String {
        let local_key = Keypair::generate_ed25519();
        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD
            .encode(local_key.to_protobuf_encoding().unwrap());
        return encoded;
    }
}
