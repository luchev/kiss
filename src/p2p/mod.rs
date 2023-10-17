use crate::p2p::{
    controller::{ISwarmController, SwarmControllerProvider},
    swarm::{ISwarm, SwarmProvider},
};
use crate::util::types::SwarmInstruction;
use runtime_injector::{constant, define_module, IntoSingleton};

use tokio::sync::{mpsc, Mutex};

pub mod controller;
mod store;
pub mod swarm;

pub fn module() -> runtime_injector::Module {
    let (sender, receiver) = mpsc::channel::<SwarmInstruction>(5);
    define_module! {
        services = [
            SwarmControllerProvider.singleton(),
            SwarmProvider.singleton(),
            constant(Mutex::new(sender)),
            constant(Mutex::new(receiver)),
        ],
        interfaces = {
            dyn ISwarmController = [ SwarmControllerProvider.singleton() ],
            dyn ISwarm = [ SwarmProvider.singleton() ],
        },
    }
}
