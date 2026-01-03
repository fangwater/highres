use crate::trade::{TradeInfo};
use log::{info,debug};
use crate::gconf::{ENGIN_CONF};
use crate::trade::{MakeDecision, CancelDecision};
use crate::stgs::{pairmm};
use crate::stgs::{sampling,sampling_v5};


use crate::spending::{PendingItem};


pub fn cb_init(tinfo:&TradeInfo) {
    if ENGIN_CONF.stg == "pairmm" {
        return pairmm::cb_init(tinfo);
    }else if ENGIN_CONF.stg == "sampling" {
        return sampling::cb_init(tinfo);
    }else if ENGIN_CONF.stg == "sampling_v5" {
        return sampling_v5::cb_init(tinfo);
    }else {
        debug!("cb_init , stg={} not impl ignore", ENGIN_CONF.stg);
    }
}

pub fn cb_filled(tinfo:&mut TradeInfo, pitem:&PendingItem, filled_amount:f64) {
    if ENGIN_CONF.stg == "pairmm" {
        return pairmm::cb_filled(tinfo, pitem, filled_amount);
    }else if ENGIN_CONF.stg == "sampling"{
        return sampling::cb_filled(tinfo, pitem, filled_amount);
    }else if ENGIN_CONF.stg == "sampling_v5"{
        return sampling_v5::cb_filled(tinfo, pitem, filled_amount);
    }
    else {
        debug!("cb_filled , stg={} not impl ignore", ENGIN_CONF.stg);
    }
}

pub fn cb_finished(ts:i64, tinfo:&mut TradeInfo, pitem:&PendingItem, is_cancel:bool) {
    if  ENGIN_CONF.stg == "pairmm" {
        return pairmm::cb_finished(ts, tinfo, pitem, is_cancel);
    } else if ENGIN_CONF.stg == "sampling"{
        return sampling::cb_finished(ts, tinfo, pitem, is_cancel);
    }else if ENGIN_CONF.stg == "sampling_v5"{
        return sampling_v5::cb_finished(ts, tinfo, pitem, is_cancel);
    }
    else {
        debug!("cb_finished , stg={} not impl ignore", ENGIN_CONF.stg);
    }
}

pub fn tick(t:&Vec<f64>, tinfo:&mut TradeInfo) ->(Vec<MakeDecision>, Vec<CancelDecision>){
    let cds:Vec<CancelDecision> = cancel(t, tinfo);
    let tds:Vec<MakeDecision> = make(t, tinfo);
    return (tds, cds);
}
    
pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    
    if ENGIN_CONF.stg == "pairmm" {
        return pairmm::make(t, tinfo);
    }else if ENGIN_CONF.stg == "sampling"{
        return sampling::make(t, tinfo);
    }else if ENGIN_CONF.stg == "sampling_v5"{
        return sampling_v5::make(t, tinfo);
    }
    else {
        panic!("stg={} not impl", ENGIN_CONF.stg);
    }
    
}


pub fn cancel(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<CancelDecision>{
    
    if ENGIN_CONF.stg == "pairmm" {
        return pairmm::cancel(t, tinfo);
    }else if ENGIN_CONF.stg == "sampling"{
        return sampling::cancel(t, tinfo);
    }else if ENGIN_CONF.stg == "sampling_v5"{
        return sampling_v5::cancel(t, tinfo);
    }else {
        panic!("stg={} not impl", ENGIN_CONF.stg);
    }
}
