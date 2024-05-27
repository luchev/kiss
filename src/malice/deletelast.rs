use super::IMalice;
use crate::{storage::IStorage, util::Res};
use async_trait::async_trait;
use log::{debug, info};
use runtime_injector::Svc;

pub struct MaliceDeleteLast {
    storage: Svc<dyn IStorage>,
}

impl MaliceDeleteLast {
    pub fn new(storage: Svc<dyn IStorage>) -> Self {
        Self { storage }
    }
}

#[async_trait]
impl IMalice for MaliceDeleteLast {
    async fn start(&self) -> Res<()> {
        log::info!("init delete random malice");
        loop {
            let mut paths = self.storage.list().await?;
            paths.sort();
            if let Some(path) = paths.into_iter().last() {
                debug!("malice deleting: {}", path);
                self.storage.remove(&path).await?;
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
