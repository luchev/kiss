use crate::settings::ISettings;
use async_trait::async_trait;
use common::grpc::keeper_grpc::keeper_grpc_client::KeeperGrpcClient;
use common::grpc::keeper_grpc::VerifyRequest;
use common::grpc::keeper_grpc::{GetRequest, PutRequest};
use common::types::Bytes;
use common::{Er, ErrorKind, Res};
use futures::executor::block_on;
use log::{info, warn};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo, Svc,
};
use std::net::SocketAddr;
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tonic::transport::Channel;

#[async_trait]
pub trait IKeeperGateway: Service {
    async fn put(&mut self, key: String, value: Bytes) -> Res<()>;
    async fn get(&mut self, path: String) -> Res<Bytes>;
    async fn verify(&mut self, path: String) -> Res<String>;
}

#[derive(Debug)]
pub struct KeeperGateway {
    client: Option<KeeperGrpcClient<Channel>>,
    addresses: Vec<SocketAddr>,
}

impl KeeperGateway {
    async fn get_client(&mut self) -> Res<&mut KeeperGrpcClient<Channel>> {
        if self.client.is_none() {
            for address in self.addresses.iter() {
                let new_client = try_connect(*address).await;
                if new_client.is_some() {
                    self.client = new_client;
                    break;
                }
            }
        }
        match self.client.as_mut() {
            Some(x) => Ok(x),
            None => Err(ErrorKind::GrpcClientIsEmpty.into()),
        }
    }
}

#[async_trait]
impl IKeeperGateway for KeeperGateway {
    async fn put(&mut self, key: String, value: Bytes) -> Res<()> {
        let client = self.get_client().await?;
        let request = tonic::Request::new(PutRequest {
            path: key,
            content: value,
        });
        let response = client.put(request).await;
        match response {
            Ok(_) => Ok(()),
            Err(e) => Err(ErrorKind::GrpcError(e).into()),
        }
    }

    async fn get(&mut self, path: String) -> Res<Bytes> {
        let client = self.get_client().await?;
        let request = tonic::Request::new(GetRequest { path });
        let response = client.get(request).await;
        match response {
            Ok(res) => Ok(res.into_inner().content),
            Err(e) => Err(ErrorKind::GrpcError(e).into()),
        }
    }

    async fn verify(&mut self, path: String) -> Res<String> {
        let client = self.get_client().await?;
        let request = tonic::Request::new(VerifyRequest { path });
        let response = client.verify(request).await;
        match response {
            Ok(res) => Ok(res.into_inner().hash),
            Err(e) => Err(ErrorKind::GrpcError(e).into()),
        }
    }
}

pub struct KeeperGatewayProvider;
#[async_trait]
impl ServiceFactory<()> for KeeperGatewayProvider {
    type Result = Mutex<KeeperGateway>;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>()?.keeper_gateway();
        let client = if let Some(address) = settings.addresses.first() {
            let handle = Handle::current();
            block_on(async { handle.spawn(try_connect(*address)).await }).map_err(|e| {
                InjectError::ActivationFailed {
                    service_info: ServiceInfo::of::<KeeperGateway>(),
                    inner: Box::<Er>::new(ErrorKind::JoinError(e).into()),
                }
            })?
        } else {
            None
        };

        let result = KeeperGateway {
            client,
            addresses: settings.addresses,
        };

        Ok(Mutex::new(result))
    }
}

interface! {
    dyn IKeeperGateway = [
        KeeperGateway,
    ]
}

async fn connect(address: SocketAddr) -> Res<KeeperGrpcClient<Channel>> {
    info!("connecting to a keeper node on address {}", address);
    KeeperGrpcClient::connect(format!("http://{}", address))
        .await
        .map_err(Er::from)
}

async fn try_connect(address: SocketAddr) -> Option<KeeperGrpcClient<Channel>> {
    info!("connecting to a keeper node on address {}", address);
    match KeeperGrpcClient::connect(format!("http://{}", address)).await {
        Ok(x) => Some(x),
        Err(e) => {
            warn!("failed connecting to node {} with error: {}", address, e);
            None
        }
    }
}
