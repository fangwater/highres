pub mod lconf;
pub mod symbolinfo;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn get_sys_ms() -> i64{
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    
    let in_ms = since_the_epoch.as_millis();
    let i64_value: i64 = in_ms as i64;
    return i64_value
}