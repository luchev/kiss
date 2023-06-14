use std::{
    env,
    net::{IpAddr, SocketAddr},
};

use common::{
    consts::{self, KEEPER_CONFIG_BASE_DIR},
    ErrorKind,
};
use config::{Config, Environment, File};
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
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
    fn storage(&self) -> Storage;
    fn grpc(&self) -> Grpc;
    fn swarm(&self) -> Swarm;
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct Settings {
    pub storage: Storage,
    pub grpc: Grpc,
    pub swarm: Swarm,
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

        Ok(
            Config::builder()
                .add_source(File::with_name(consts::KEEPER_CONFIG_BASE))
                .add_source(
                    File::with_name(&format!("{}/{}", KEEPER_CONFIG_BASE_DIR, env_conf))
                        .required(false),
                )
                .add_source(
                    Environment::with_prefix("KISS")
                        .try_parsing(true)
                        .separator("_")
                        .list_separator(":"),
                )
                .build()
                .map_err(|err| ErrorKind::ConfigErr(err))
                .unwrap() // TODO remove
                .try_deserialize()
                .map_err(|err| ErrorKind::ConfigErr(err))
                .unwrap(), // TODO remove
        )
    }
}

interface! {
    dyn ISettings = [
        Settings,
    ]
}
