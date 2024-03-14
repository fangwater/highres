use crate::trade::{TradeInfo};
//use log::{info};
use crate::gconf::{ENGIN_CONF};
use crate::trade::{MakeDecision, CancelDecision};
use crate::stgs::{arbmm, arbmt_sampling, arbmm2};



pub fn tick(t:&Vec<f64>, tinfo:&mut TradeInfo) ->(Vec<MakeDecision>, Vec<CancelDecision>){
    let tds:Vec<MakeDecision> = make(t, tinfo);
    let cds:Vec<CancelDecision> = cancel(t, tinfo);
    return (tds, cds);
}
    
pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    
    if ENGIN_CONF.stg == "arbmm" {
        return arbmm::make(t, tinfo);
    }
    else if ENGIN_CONF.stg == "arbmt_sampling" {
        return arbmt_sampling::make(t, tinfo);
    }
    else if ENGIN_CONF.stg == "arbmm2" {
        return arbmm2::make(t, tinfo);
    }
    else {
        panic!("stg={} not impl", ENGIN_CONF.stg);
    }
    
}


pub fn cancel(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<CancelDecision>{
    
    if ENGIN_CONF.stg == "arbmm" {
        return arbmm::cancel(t, tinfo);
    }
    else if ENGIN_CONF.stg == "arbmt_sampling" {
        return arbmt_sampling::cancel(t, tinfo);
    }
    else if ENGIN_CONF.stg == "arbmm2" {
        return arbmm2::cancel(t, tinfo);
    }
    else {
        panic!("stg={} not impl", ENGIN_CONF.stg);
    }
}
