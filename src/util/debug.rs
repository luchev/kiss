use std::time::{SystemTime, UNIX_EPOCH};

use log::info;

pub fn print_now(message: &str) {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    info!("{} trigger: {}", since_the_epoch.as_millis(), message);
}
