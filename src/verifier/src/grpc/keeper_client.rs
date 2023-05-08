use crate::{settings::ISettings, types::Bytes};
use async_trait::async_trait;
use keeper_grpc::keeper_grpc_client::KeeperGrpcClient;
use libp2p::dns::ResolveError;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use tokio::sync::Mutex;
use tonic::transport::Channel;
use keeper_grpc::{PutRequest, GetRequest};
use common::{Res, ErrorKind};

mod keeper_grpc {
    tonic::include_proto!("keeper_grpc");
}

#[async_trait]
pub trait IKeeperGateway: Service {
    async fn put(&mut self, key: String, value: String) -> Res<()>;
    async fn get(&mut self, key: String) -> Res<Bytes>;
    async fn connect(&mut self);
}

#[derive(Debug)]
pub struct KeeperGateway {
    address: SocketAddr,
    client: Mutex<Option<KeeperGrpcClient<Channel>>>,
}

#[async_trait]
impl IKeeperGateway for KeeperGateway {
    async fn put(&mut self, key: String, value: String) -> Res<()> {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();
        let request = tonic::Request::new(PutRequest{
            path: key,
            content: value.into_bytes(),
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
        let request = tonic::Request::new(GetRequest {
            path: key,
        });
        let response = client.get(request).await;
        match response {
            Ok(res) => Ok(res.into_inner().content),
            Err(e) => Err(ErrorKind::GrpcError(e).into()),
        }
    }

    async fn connect(&mut self) {
        let mut client = self.client.lock().await;
        if client.is_none() {
            println!("http://{}", self.address);
            *client = Some(
                KeeperGrpcClient::connect(format!("http://{}", self.address))
                    .await
                    .expect("failed to connect to keeper node on address"),
            );
            info!("connected to a keeper node on address {}", self.address);
        }
    }
}

pub struct KeeperGatewayProvider;
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
        let result = KeeperGateway {
            address: settings.addresses[0],
            client: Mutex::new(None),
        };

        Ok(Mutex::new(result))
    }
}

interface! {
    dyn IKeeperGateway = [
        KeeperGateway,
    ]
}
