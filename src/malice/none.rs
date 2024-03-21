use super::IMalice;
use crate::util::Res;
use async_trait::async_trait;

#[derive(Debug, Default)]
pub struct MaliceNone {}

#[async_trait]
impl IMalice for MaliceNone {
    async fn start(&self) -> Res<()> {
        log::info!("init no malice");
        Ok(())
    }
}
