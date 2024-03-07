use std::{
    env,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use crate::util::{
    consts::{self, CONFIG_DIR},
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
pub enum Storage {
    Local {
        path: String,
        #[serde(default = "Storage::default_create")]
        create: bool,
    },
    Docker,
}

impl Default for Storage {
    fn default() -> Self {
        Self::Local {
            path: "data".to_string(),
            create: true,
        }
    }
}

impl Storage {
    fn default_create() -> bool {
        true
    }
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
    fn storage(&self) -> Storage;
    fn grpc(&self) -> Grpc;
    fn swarm(&self) -> Swarm;
    fn ledger(&self) -> Ledger;
    fn malicious_behavior(&self) -> MaliciousBehavior;
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ledger {
    Immudb {
        username: String,
        password: String,
        address: SocketAddr,
    },
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MaliciousBehavior {
    None,
    DeleteAll,
    DeleteRandom(usize),
}

impl Default for MaliciousBehavior {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Grpc {
    pub port: u16,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Settings {
    pub storage: Storage,
    pub grpc: Grpc,
    pub swarm: Swarm,
    pub ledger: Ledger,
    pub malicious_behavior: MaliciousBehavior,
}

impl ISettings for Settings {
    fn storage(&self) -> Storage {
        self.storage.clone()
    }

    fn grpc(&self) -> Grpc {
        self.grpc.clone()
    }

    fn swarm(&self) -> Swarm {
        self.swarm.clone()
    }

    fn ledger(&self) -> Ledger {
        self.ledger.clone()
    }

    fn malicious_behavior(&self) -> MaliciousBehavior {
        self.malicious_behavior.clone()
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
            .add_source(File::with_name(consts::BASE_CONFIG))
            .add_source(File::with_name(&format!("{}/{}", CONFIG_DIR, env_conf)).required(false))
            .add_source(
                Environment::with_prefix("KISS")
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(":"),
            )
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
