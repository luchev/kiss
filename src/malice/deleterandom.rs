use super::IMalice;
use crate::util::Res;
use async_trait::async_trait;

#[derive(Debug, Default)]
pub struct MaliceDeleteRandom {}

#[async_trait]
impl IMalice for MaliceDeleteRandom {
    async fn start(&self) -> Res<()> {
        log::info!("init delete random malice");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            // do stuff
        }
    }
}
