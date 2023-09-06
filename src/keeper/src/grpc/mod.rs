mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}
use crate::p2p::controller::ISwarmController;
use crate::settings::ISettings;
use crate::storage::IStorage;
use async_trait::async_trait;
use common::consts::{GRPC_TIMEOUT, LOCALHOST};
use common::hasher::hash;
use common::{ErrorKind, Res};
use keeper_grpc::keeper_grpc_server::KeeperGrpc;
use keeper_grpc::keeper_grpc_server::KeeperGrpcServer;
use keeper_grpc::{GetRequest, GetResponse, PutRequest, PutResponse};
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

use self::keeper_grpc::{VerifyRequest, VerifyResponse};

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
            .add_service(KeeperGrpcServer::new(self.inner.clone()))
            .serve_with_incoming(TcpListenerStream::new(listener))
            .await?;

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
        let res = self
            .swarm_controller
            .set(request.path, request.content)
            .await;
        info!("kad result {:?}", res);
        // self.storage
        //     .put(request.path.into(), request.content)
        //     .await
        //     .map_err(|e| match e.kind() {
        //         ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
        //         _ => Status::unknown("Unknown storage error".to_string()),
        //     })?;

        let reply = PutResponse {};
        Ok(Response::new(reply))
    }

    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> std::result::Result<Response<GetResponse>, Status> {
        let request = request.into_inner();
        info!("received a get request for {}", request.path);
        let res = self.swarm_controller.get(request.path).await;
        info!("kad result {:?}", res);
        let content =
            res.map_err(|e| Status::not_found(format!("failed getting from swarm: {}", e)))?;
        // let content = self
        //     .storage
        //     .get(request.path.into())
        //     .await
        //     .map_err(|e| match e.kind() {
        //         ErrorKind::StoragePutFailed(e) => Status::invalid_argument(e.to_string()),
        //         _ => Status::unknown("Unknown storage error".to_string()),
        //     })?;

        let reply = GetResponse { content };
        Ok(Response::new(reply))
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

// use base64::Engine;
// use libp2p_identity::Keypair;
// impl GrpcHandler {
//     async fn generate_keypair(&self) -> String {
//         let local_key = Keypair::generate_ed25519();
//         let encoded = base64::engine::general_purpose::STANDARD_NO_PAD
//             .encode(local_key.to_protobuf_encoding().unwrap());
//         return encoded;
//     }
// }
