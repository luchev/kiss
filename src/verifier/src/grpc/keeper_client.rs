use crate::{settings::ISettings, types::Bytes};
use async_trait::async_trait;
use common::{ErrorKind, Res};
use futures::executor::block_on;
use keeper_grpc::keeper_grpc_client::KeeperGrpcClient;
use keeper_grpc::{GetRequest, PutRequest};
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use tokio::runtime::Handle;
use tokio::{sync::Mutex};
use tonic::transport::Channel;

mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}

#[async_trait]
pub trait IKeeperGateway: Service {
    async fn put(&mut self, key: String, value: Bytes) -> Res<()>;
    async fn get(&mut self, key: String) -> Res<Bytes>;
}

#[derive(Debug)]
pub struct KeeperGateway {
    client: Mutex<Option<KeeperGrpcClient<Channel>>>,
}

#[async_trait]
impl IKeeperGateway for KeeperGateway {
    async fn put(&mut self, key: String, value: Bytes) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();
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

    async fn get(&mut self, key: String) -> Res<Bytes> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();
        let request = tonic::Request::new(GetRequest { path: key });
        let response = client.get(request).await;
        match response {
            Ok(res) => Ok(res.into_inner().content),
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
        let settings = injector
            .get::<Svc<dyn ISettings>>()
            .expect("settings were not provided")
            .keeper_gateway();
        if settings.addresses.len() == 0 {
            panic!("no keeper nodes were provided");
        }
        let address = settings.addresses[0];
        let handle = Handle::current();
        let client = block_on(async { handle.spawn(connect(address)).await.unwrap() });

        let result = KeeperGateway {
            client: Mutex::new(Some(client)),
        };

        Ok(Mutex::new(result))
    }
}

interface! {
    dyn IKeeperGateway = [
        KeeperGateway,
    ]
}

async fn connect(address: SocketAddr) -> KeeperGrpcClient<Channel> {
    let client = KeeperGrpcClient::connect(format!("http://{}", address))
        .await
        .expect("failed to connect to keeper node on address");

    info!("connected to a keeper node on address {}", address);
    client
}
