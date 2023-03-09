use common::{
    consts,
    errors::{ErrorKind, Result},
};
use config::{Config, File};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StorageType {
    Local {
        path: String,
        #[serde(default = "StorageType::default_create")]
        create: bool,
    },
    Docker,
}

impl Default for StorageType {
    fn default() -> Self {
        Self::Local {
            path: "data".to_string(),
            create: true,
        }
    }
}

impl StorageType {
    fn default_create() -> bool {
        true
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct Settings {
    pub storage: StorageType,
}

impl Settings {
    pub fn new() -> Result<Self> {
        Config::builder()
            .add_source(File::with_name(consts::CONFIG_BASE))
            .build()
            .map_err(|err| ErrorKind::ConfigErr(err))?
            .try_deserialize()
            .map_err(|err| ErrorKind::ConfigErr(err).into())
    }
}
