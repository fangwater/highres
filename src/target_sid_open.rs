use log::{info};
use crate::gconf::{ENGIN_CONF};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Duration;



lazy_static! {
    pub static ref TARGET_SID_OPEN: HashMap<i32, RwLock<f64>> = {
        let mut map = HashMap::new();
        
        for sid in ENGIN_CONF.vsids.iter() {
            map.insert(*sid, RwLock::new(0.));
        }
        map
    };
}

pub fn add(sid:&i32, amount:f64) {
    
    println!("{}",sid);
    if let Some(mut camount) = TARGET_SID_OPEN[sid].try_write_for(Duration::from_secs(1)) {
        *camount+=amount;
    }
   
}


pub fn get(sid:&i32) -> f64{

    
    let amount:f64 = *(TARGET_SID_OPEN[sid].read());
    return amount;
}


