mod errors;
pub mod consts;
pub mod hasher;
pub mod types;

pub use errors::Result as Res;
pub use errors::Error as Er;
pub use errors::die;
pub use errors::ErrorKind;
