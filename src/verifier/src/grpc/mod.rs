use crate::settings::ISettings;
use async_trait::async_trait;
use common::consts::{GRPC_TIMEOUT, LOCALHOST};
use common::{ErrorKind, Res};
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use std::time::Duration;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use verifier_grpc::verifier_grpc_server::{VerifierGrpc, VerifierGrpcServer};
use verifier_grpc::{RetrieveRequest, RetrieveResponse, StoreRequest, StoreResponse};

use self::keeper_client::IKeeperGateway;

mod verifier_grpc {
    tonic::include_proto!("verifier_grpc");
}
pub mod keeper_client;

interface! {
    dyn IGrpcHandler = [
        GrpcHandler,
    ]
}

pub struct GrpcHandlerProvider;
impl ServiceFactory<()> for GrpcHandlerProvider {
    type Result = GrpcHandler;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let port = injector.get::<Svc<dyn ISettings>>().unwrap().grpc().port;
        let keeper_gateway = injector
            .get::<Svc<dyn IKeeperGateway>>()
            .expect("keeper gateway not provided");

        Ok(GrpcHandler {
            inner: Inner { keeper_gateway },
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
    keeper_gateway: Svc<dyn IKeeperGateway>,
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

        info!("grpc listening on {}", addr);

        let middleware = tower::ServiceBuilder::new()
            .timeout(Duration::from_secs(GRPC_TIMEOUT))
            .layer(tonic::service::interceptor(|req| Ok(req)))
            .into_inner();

        Server::builder()
            .layer(middleware)
            .add_service(VerifierGrpcServer::new(self.inner.clone()))
            .serve(addr)
            .await
            .map_err(|e| ErrorKind::GrpcServerStartFailed(e))?;

        Ok(())
    }
}

#[async_trait]
impl VerifierGrpc for Inner {
    async fn store(
        &self,
        request: Request<StoreRequest>,
    ) -> std::result::Result<Response<StoreResponse>, Status> {
        let request = request.into_inner();
        info!("received a store request for {}", request.name);
        todo!();
        // let reply = StoreResponse {};
        // Ok(Response::new(reply))
    }

    async fn retrieve(
        &self,
        request: Request<RetrieveRequest>,
    ) -> std::result::Result<Response<RetrieveResponse>, Status> {
        let request = request.into_inner();
        info!("received a get request for {}", request.name);
        todo!();
        // let reply = RetrieveResponse { name: "", data: vec![] };
        // Ok(Response::new(reply))
    }
}
