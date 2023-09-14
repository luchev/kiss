#![recursion_limit = "256"]
pub mod consts;
mod errors;
pub mod grpc;
pub mod hasher;
pub mod types;

pub use errors::die;
pub use errors::Error as Er;
pub use errors::ErrorKind;
pub use errors::Result as Res;
