use super::IMalice;
use crate::{storage::IStorage, util::Res};
use async_trait::async_trait;
use runtime_injector::Svc;

#[derive()]
pub struct MaliceDeleteAll {
    storage: Svc<dyn IStorage>,
}

impl MaliceDeleteAll {
    pub fn new(storage: Svc<dyn IStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl IMalice for MaliceDeleteAll {
    async fn start(&self) -> Res<()> {
        log::info!("init delete all malice");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            // do stuff
        }
    }
}
