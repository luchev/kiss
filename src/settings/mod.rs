use crate::{
    p2p::peer_id::{keypair_to_base64_proto, keypair_with_leading_zeros},
    util::{
        consts::{self, CONFIG_DIR},
        Er, ErrorKind,
    },
};
use config::{Config, Environment, File};
use rand::distributions::Alphanumeric;
use rand::thread_rng;
use rand::Rng;
use runtime_injector::{
    interface, InjectError, InjectResult, Injector, RequestInfo, Service, ServiceFactory,
    ServiceInfo,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

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
    pub leading_zeros: usize,
    pub port: u16,
    pub bootstrap: Vec<SocketAddr>,
}

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Verifier {
    pub enabled: bool,
}

pub trait ISettings: Service {
    fn storage(&self) -> Storage;
    fn grpc(&self) -> Grpc;
    fn swarm(&self) -> Swarm;
    fn ledger(&self) -> Ledger;
    fn malicious_behavior(&self) -> MaliciousBehavior;
    fn verifier(&self) -> Verifier;
    fn por(&self) -> Por;
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
    DeleteLast,
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

#[derive(Default, Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Por {
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Settings {
    pub storage: Storage,
    pub grpc: Grpc,
    pub swarm: Swarm,
    pub ledger: Ledger,
    pub malicious_behavior: Option<MaliciousBehavior>,
    pub verifier: Verifier,
    pub por: Por,
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
        self.malicious_behavior.clone().unwrap_or_default()
    }

    fn verifier(&self) -> Verifier {
        self.verifier.clone()
    }

    fn por(&self) -> Por {
        self.por.clone()
    }
}

fn random_string(len: usize) -> String {
    String::from_utf8(
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(len)
            .collect::<Vec<_>>(),
    )
    .unwrap_or_default()
}

fn random_port() -> u16 {
    rand::thread_rng().gen_range(5000..10000)
}

impl Settings {
    pub fn new(config_name: &str) -> Self {
        Self {
            storage: Storage::Local {
                path: format!("{}/{}", consts::DATA_DIR, config_name),
                create: true,
            },
            grpc: Grpc {
                port: random_port(),
            },
            swarm: Swarm {
                keypair: keypair_to_base64_proto(keypair_with_leading_zeros(
                    consts::DEFAULT_LEADING_ZEROS,
                ))
                .into(),
                leading_zeros: consts::DEFAULT_LEADING_ZEROS,
                port: random_port(),
                bootstrap: vec![],
            },
            ledger: Ledger::Immudb {
                username: "immudb".to_string(),
                password: "immudb".to_string(),
                address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 3322)),
            },
            malicious_behavior: MaliciousBehavior::None.into(),
            verifier: Verifier { enabled: true },
            por: Por { enabled: true },
        }
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

        if !std::path::Path::new(&format!("{}/{}.yaml", CONFIG_DIR, env_conf)).exists() {
            log::info!("creating new config file for env: {}", env_conf);
            let settings = Settings::new(&env_conf);
            let serialized = serde_yaml::to_string(&settings).unwrap_or_default();
            std::fs::write(format!("{}/{}.yaml", CONFIG_DIR, env_conf), serialized)
                .unwrap_or_default();
        }

        let mut builder = Config::builder();
        if std::path::Path::new(consts::BASE_CONFIG).exists() {
            builder = builder.add_source(File::with_name(consts::BASE_CONFIG));
        }

        builder
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
