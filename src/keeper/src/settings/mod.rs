use common::{
    consts,
    errors::{ErrorKind, Result},
};
use config::{Config, File};
use runtime_injector::{interface, Service};
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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct Grpc {
    pub port: u16,
}

impl Default for Grpc {
    fn default() -> Self {
        Grpc { port: 5005 }
    }
}

pub trait Settings: Service {
    fn storage(&self) -> Storage;
    fn grpc(&self) -> Grpc;
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct SettingsImpl {
    pub storage: Storage,
    pub grpc: Grpc,
}

impl Settings for SettingsImpl {
    fn storage(&self) -> Storage {
        self.storage.clone()
    }

    fn grpc(&self) -> Grpc {
        self.grpc.clone()
    }
}

impl SettingsImpl {
    pub fn new() -> Result<Self> {
        Config::builder()
            .add_source(File::with_name(consts::CONFIG_BASE))
            .build()
            .map_err(|err| ErrorKind::ConfigErr(err))?
            .try_deserialize()
            .map_err(|err| ErrorKind::ConfigErr(err).into())
    }

    pub fn constructor() -> impl Fn() -> Self {
        || {
            Config::builder()
                .add_source(File::with_name(consts::CONFIG_BASE))
                .build()
                .map_err(|err| ErrorKind::ConfigErr(err))
                .unwrap()
                // .map_err(|err| ErrorKind::ConfigErr(err))?
                .try_deserialize()
                .unwrap()
        }
    }
}

interface! {
    dyn Settings = [
        SettingsImpl,
    ]
}
