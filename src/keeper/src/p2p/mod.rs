use crate::p2p::{
    controller::{ISwarmController, Instruction, SwarmControllerProvider},
    swarm::{ISwarm, SwarmProvider},
};
use runtime_injector::{constant, define_module, IntoSingleton};

use tokio::sync::{mpsc, Mutex};

pub mod controller;
pub mod swarm;

pub fn module() -> runtime_injector::Module {
    let (sender, receiver) = mpsc::channel::<Instruction>(5);
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

// fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String) {
//     let mut args = line.split(' ');

//     match args.next() {
//         Some("GET") => {
//             let key = {
//                 match args.next() {
//                     Some(key) => Key::new(&key),
//                     None => {
//                         eprintln!("Expected key");
//                         return;
//                     }
//                 }
//             };
//             kademlia.get_record(key);
//         }
//         Some("GET_PROVIDERS") => {
//             let key = {
//                 match args.next() {
//                     Some(key) => Key::new(&key),
//                     None => {
//                         eprintln!("Expected key");
//                         return;
//                     }
//                 }
//             };
//             kademlia.get_providers(key);
//         }
//         Some("PUT") => {
//             let key = {
//                 match args.next() {
//                     Some(key) => Key::new(&key),
//                     None => {
//                         eprintln!("Expected key");
//                         return;
//                     }
//                 }
//             };
//             let value = {
//                 match args.next() {
//                     Some(value) => value.as_bytes().to_vec(),
//                     None => {
//                         eprintln!("Expected value");
//                         return;
//                     }
//                 }
//             };
//             let record = Record {
//                 key,
//                 value,
//                 publisher: None,
//                 expires: None,
//             };
//             kademlia
//                 .put_record(record, Quorum::One)
//                 .expect("Failed to store record locally.");
//         }
//         Some("PUT_PROVIDER") => {
//             let key = {
//                 match args.next() {
//                     Some(key) => Key::new(&key),
//                     None => {
//                         eprintln!("Expected key");
//                         return;
//                     }
//                 }
//             };

//             kademlia
//                 .start_providing(key)
//                 .expect("Failed to start providing key");
//         }
//         _ => {
//             eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
//         }
//     }
// }
