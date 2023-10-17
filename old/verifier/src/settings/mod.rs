use std::{
    env,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use common::{
    consts::{self, VERIFIER_CONFIG_BASE_DIR},
    Er, ErrorKind,
};
use config::{Config, Environment, File};
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ledger {
    Immudb {
        username: String,
        password: String,
        address: SocketAddr,
    },
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
pub struct KeeperGateway {
    pub addresses: Vec<SocketAddr>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::Immudb {
            username: "".to_string(),
            password: "".to_string(),
            address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3322)),
        }
    }
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Grpc {
    pub port: u16,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Swarm {
    #[serde(default)]
    pub keypair: Option<String>,
    pub port: u16,
    pub bootstrap: Vec<SocketAddr>,
}

pub trait ISettings: Service {
    fn ledger(&self) -> Ledger;
    fn grpc(&self) -> Grpc;
    fn keeper_gateway(&self) -> KeeperGateway;
    fn swarm(&self) -> Swarm;
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Settings {
    pub ledger: Ledger,
    pub grpc: Grpc,
    pub keeper_gateway: KeeperGateway,
    pub swarm: Swarm,
}

impl ISettings for Settings {
    fn ledger(&self) -> Ledger {
        self.ledger.clone()
    }

    fn grpc(&self) -> Grpc {
        self.grpc.clone()
    }

    fn keeper_gateway(&self) -> KeeperGateway {
        self.keeper_gateway.clone()
    }

    fn swarm(&self) -> Swarm {
        self.swarm.clone()
    }
}

pub struct SettingsProvider;
impl ServiceFactory<()> for SettingsProvider {
    type Result = Settings;

    fn invoke(
        &mut self,
        _injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let env_conf = env::var("ENV").unwrap_or_else(|_| "dev".into());

        Config::builder()
            .add_source(File::with_name(consts::VERIFIER_CONFIG_BASE))
            .add_source(
                File::with_name(&format!("{}/{}", VERIFIER_CONFIG_BASE_DIR, env_conf))
                    .required(false),
            )
            .add_source(Environment::with_prefix("KISS"))
            .build()
            .map_err(|err| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<Settings>(),
                inner: Box::<Er>::new(ErrorKind::ConfigErr(err).into()),
            })?
            .try_deserialize()
            .map_err(|err| InjectError::ActivationFailed {
                service_info: ServiceInfo::of::<Settings>(),
                inner: Box::<Er>::new(ErrorKind::ConfigErr(err).into()),
            })
    }
}

interface! {
    dyn ISettings = [
        Settings,
    ]
}
