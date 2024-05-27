mod deleteall;
mod deletelast;
mod none;

use crate::settings::{ISettings, MaliciousBehavior};
use crate::storage::IStorage;
use crate::util::Res;
use async_trait::async_trait;
use runtime_injector::{
    interface, InjectResult, Injector, RequestInfo, Service, ServiceFactory, Svc,
};

use self::deleteall::MaliceDeleteAll;
use self::deletelast::MaliceDeleteLast;
use self::none::MaliceNone;

#[async_trait]
pub trait IMalice: Service {
    async fn start(&self) -> Res<()>;
}

pub struct MaliceProvider;
impl ServiceFactory<()> for MaliceProvider {
    type Result = Box<dyn IMalice>;

    fn invoke(
        &mut self,
        injector: &Injector,
        _request_info: &RequestInfo,
    ) -> InjectResult<Self::Result> {
        let settings = injector.get::<Svc<dyn ISettings>>()?.malicious_behavior();
        let storage = injector.get::<Svc<dyn IStorage>>()?;

        match settings {
            MaliciousBehavior::None => Ok(Box::<MaliceNone>::default()),
            MaliciousBehavior::DeleteAll => Ok(Box::new(MaliceDeleteAll::new(storage))),
            MaliciousBehavior::DeleteLast => Ok(Box::new(MaliceDeleteLast::new(storage))),
        }
    }
}

interface! {
    dyn IMalice = [
        MaliceNone,
    ]
}
