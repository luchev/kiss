use super::IMalice;
use crate::{storage::IStorage, util::Res};
use async_trait::async_trait;
use log::{debug, info};
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
        info!("init delete all malice");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            let paths = self.storage.list().await?;
            for path in paths {
                debug!("malice deleting: {}", path);
                self.storage.remove(&path).await?;
            }
        }
    }
}
