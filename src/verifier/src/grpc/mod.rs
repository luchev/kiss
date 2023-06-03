use self::keeper_client::KeeperGateway;
use crate::grpc::keeper_client::IKeeperGateway;
use crate::ledger::ILedger;
use crate::ledger::ImmuLedger;
use crate::settings::ISettings;
use async_trait::async_trait;
use common::consts::{GRPC_TIMEOUT, LOCALHOST};
use common::hasher::hash;
use common::{hasher, ErrorKind, Res};
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::Mutex;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use verifier_grpc::verifier_grpc_server::{VerifierGrpc, VerifierGrpcServer};
use verifier_grpc::{RetrieveRequest, RetrieveResponse, StoreRequest, StoreResponse};

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
        let keeper_gateway: Svc<Mutex<KeeperGateway>> =
            injector.get().expect("keeper gateway not provided");
        let ledger: Svc<Mutex<ImmuLedger>> = injector.get().expect("ledger not provided");

        Ok(GrpcHandler {
            inner: Inner {
                keeper_gateway,
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
    keeper_gateway: Svc<Mutex<KeeperGateway>>,
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
        let file_hash = hash(&request.content);
        info!("{}", file_hash);
        let mut ledger = self.ledger.lock().await;
        let file_uuid = ledger
            .create_contract(file_hash, request.ttl)
            .await
            .unwrap();

        let res = self
            .keeper_gateway
            .lock()
            .await
            .put(file_uuid.clone(), request.content)
            .await;

        match res {
            Ok(_) => Ok(Response::new(StoreResponse { name: file_uuid })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn retrieve(
        &self,
        request: Request<RetrieveRequest>,
    ) -> std::result::Result<Response<RetrieveResponse>, Status> {
        let request = request.into_inner();
        info!("received a get request for {}", request.name);
        let mut ledger = self.ledger.lock().await;
        let contract = ledger.get_contract(request.name.clone()).await.unwrap();

        let res = self
            .keeper_gateway
            .lock()
            .await
            .get(request.name.clone())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let file_hash = hasher::hash(&res);
        if file_hash != contract.file_hash {
            return Err(Status::data_loss("file has been modified"));
        }

        Ok(Response::new(RetrieveResponse {
            name: request.name,
            content: res,
        }))
    }
}
