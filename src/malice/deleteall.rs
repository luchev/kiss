use crate::{bench::Bench, storage::IStorage, util::Res};
use async_trait::async_trait;
use log::{debug, info};
use runtime_injector::Svc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use super::IMalice;

pub struct MaliceDeleteAll {
    storage: Svc<dyn IStorage>,
    bench: Svc<Mutex<Bench>>,
}

impl MaliceDeleteAll {
    pub fn new(storage: Svc<dyn IStorage>, bench: Svc<Mutex<Bench>>) -> Self {
        Self { storage, bench }
    }
}

#[async_trait]
impl IMalice for MaliceDeleteAll {
    async fn start(&self) -> Res<()> {
        info!("init delete all malice");
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        loop {
            let paths = self.storage.list().await?;
            for path in paths {
                debug!("malice deleting: {}", path);
                self.storage.remove(&path).await?;
                self.bench
                    .lock()
                    .await
                    .deleted_files
                    .insert(path.to_string(), SystemTime::now());
                print_now(format!("corrupted {}", path).as_str());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

fn print_now(message: &str) {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    info!("{} at {}", message, since_the_epoch.as_millis());
}
