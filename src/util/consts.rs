use time::Duration;

pub const DOCKER_SOCKET_PATH: &str = "unix:///var/run/docker.sock";
pub const CONFIG_DIR: &str = "config";
pub const DATA_DIR: &str = "data";
pub const BASE_CONFIG: &str = "config/base.yaml";
pub const LOCALHOST: &str = "[::1]";
pub const GRPC_TIMEOUT: u64 = 30;
pub const DEFAULT_LEADING_ZEROS: usize = 2;
pub const AUDIT_REWARD: i64 = 1;
pub const AUDIT_PENALTY: i64 = 5;
pub const VERIFICATION_TIMEOUT: Duration = Duration::seconds(5);
pub const NUM_PEERS: u128 = 3;
