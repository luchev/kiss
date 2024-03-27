use crate::util::types::CommandToSwarm;
use crate::{
    p2p::{
        controller::{ISwarmController, SwarmControllerProvider},
        swarm::{ISwarm, SwarmProvider},
    },
    util::types::CommandToController,
};
use runtime_injector::{constant, define_module, IntoSingleton};

use tokio::sync::{mpsc, Mutex};

pub mod controller;
mod memorystore;
pub mod peer_id;
mod store;
pub mod swarm;

pub fn module() -> runtime_injector::Module {
    let (sender_from_controller, receiver_in_swarm) = mpsc::channel::<CommandToSwarm>(5);
    define_module! {
        services = [
            SwarmControllerProvider.singleton(),
            SwarmProvider.singleton(),
            constant(Mutex::new(sender_from_controller)),
            constant(Mutex::new(receiver_in_swarm)),
        ],
        interfaces = {
            dyn ISwarmController = [ SwarmControllerProvider.singleton() ],
            dyn ISwarm = [ SwarmProvider.singleton() ],
        },
    }
}
