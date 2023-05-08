use crate::{
    immudb_grpc::{
        immu_service_client::ImmuServiceClient, KeyRequest, KeyValue, LoginRequest, SetRequest,
    },
    settings::{ISettings, Ledger},
};
use async_trait::async_trait;
use log::info;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};
use std::net::SocketAddr;
use tokio::sync::Mutex;
use tonic::{metadata::MetadataMap, transport::Channel, Extensions};

#[async_trait]
pub trait ILedger: Service {
    async fn set(&mut self, key: String, value: String);
    async fn get(&mut self, key: String) -> String;
    async fn login(&mut self);
}

#[derive(Debug)]
pub struct ImmuLedger {
    token: String,
    address: SocketAddr,
    username: String,
    password: String,
    client: Mutex<Option<ImmuServiceClient<Channel>>>,
}

#[async_trait]
impl ILedger for ImmuLedger {
    async fn set(&mut self, key: String, value: String) {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            SetRequest {
                k_vs: vec![KeyValue {
                    key: key.as_bytes().to_vec(),
                    value: value.as_bytes().to_vec(),
                    metadata: None,
                }],
                no_wait: false,
                preconditions: vec![],
            },
        );
        let _response = client.set(request).await.unwrap();
    }

    async fn get(&mut self, key: String) -> String {
        let mut client = self.client.lock().await;
        let client = client.as_mut().unwrap();

        let mut map = MetadataMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", self.token).parse().unwrap(),
        );

        let request = tonic::Request::from_parts(
            map,
            Extensions::default(),
            KeyRequest {
                key: key.as_bytes().to_vec(),
                no_wait: false,
                at_revision: 0,
                at_tx: 0,
                since_tx: 0,
            },
        );
        let response = client.get(request).await.unwrap();
        String::from_utf8(response.into_inner().value).unwrap()
    }

    async fn login(&mut self) {
        let mut client = self.client.lock().await;
        if client.is_none() {
            *client = Some(
                ImmuServiceClient::connect(format!("http://{}", self.address))
                    .await
                    .expect("Failed to connect to immudb"),
            );
        }

        let client = client.as_mut().expect("invalid immudb client");
        let request = tonic::Request::new(LoginRequest {
            user: self.username.as_bytes().to_vec(),
            password: self.password.as_bytes().to_vec(),
        });
        let response = client.login(request).await.expect("failed to login to immudb");

        self.token = response.into_inner().token;
        info!("Logged into immudb");
    }
}

pub struct LedgerProvider;
impl ServiceFactory<()> for LedgerProvider {
    type Result = Mutex<ImmuLedger>;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>().unwrap().ledger();
        let result = match settings {
            Ledger::Immudb {
                username,
                password,
                address,
            } => ImmuLedger {
                username,
                password,
                address,
                token: "".to_string(),
                client: Mutex::new(None),
            },
        };

        Ok(Mutex::new(result))

        // let mut client = ImmuServiceClient::connect("http://localhost:3322")
        //     .await
        //     .unwrap();

        // let request = tonic::Request::new(LoginRequest {
        //     user: b"immudb".to_vec(),
        //     password: b"immudb".to_vec(),
        // });

        // let response = client.login(request).await.unwrap();
        // let token = response.into_inner().token;

        // let mut map = MetadataMap::new();
        // map.insert(
        //     "authorization",
        //     format!("Bearer {}", token).parse().unwrap(),
        // );

        // let request = tonic::Request::from_parts(
        //     map,
        //     Extensions::default(),
        //     SetRequest {
        //         k_vs: vec![KeyValue {
        //             key: b"abc".to_vec(),
        //             value: b"myVALUE".to_vec(),
        //             metadata: None,
        //         }],
        //         no_wait: false,
        //         preconditions: vec![],
        //     },
        // );
    }
}

interface! {
    dyn ILedger = [
        ImmuLedger,
    ]
}
