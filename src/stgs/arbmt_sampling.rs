use log::{info};
use std::collections::HashMap;
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,ARBMTSAMPLING_CONF};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::spending::{get_pending_num_from_key, get_pending_num_from_key_bid, get_pending_num_from_key_ask};
use crate::spending::{S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS, PendingItem};
//use ordered_float::OrderedFloat;
use std::time::Duration;
//use crate::symbolinfo;
//use markets::Market;
use crate::symbolinfo::get_market;


use rust_decimal::Decimal;
use std::str::FromStr;
use rust_decimal::prelude::*;


lazy_static! {
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);
}



fn adjust_price(price: f64, tick_size: f64,tick_size_keep:u32, is_buy_order: bool)  -> f64 {
    let remainder = price % tick_size;
    let adjustment = if remainder != 0.0 {
        if is_buy_order {
            // Buy order: round down
            -remainder
        } else {
            // Sell order: round up
            tick_size - remainder
        }
    } else {
        0.0
    };
    let value = price + adjustment;
    let multiplier = 10_f64.powi(tick_size_keep as i32);
    (value * multiplier).round() / multiplier
}

pub fn is_cancel(t:&Vec<f64>, tinfo:&mut TradeInfo, pitem:&PendingItem, sid:&i32) -> bool{
    //计算信号
    
    if (t[0]*1000.) as i64 - pitem.create_ts > ARBMTSAMPLING_CONF.max_order_keep_s*1000 as i64 {
        info!("pending last too long o={} c={}", t[0] as i64, pitem.create_ts);
        return true
    }
    return false
}


pub fn cancel(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<CancelDecision>{
    let mut cds:Vec<CancelDecision> = Vec::new();
    let msts = (t[0] * 1000.) as i64;

    
    
    for sid in ENGIN_CONF.vsids.iter() {
    
        if let Some(porders) = S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(Duration::from_secs(1)) {
            for (_price, vpitem) in porders.phash.iter() {
                for pitem in vpitem.iter() {
                     if is_cancel(t, tinfo, pitem, &sid) {
                        let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                        cds.push(v);
                    }
                }

            }
        }
        
        if let Some(porders) = S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1)) {
            for (_price, vpitem) in porders.phash.iter() {
                for pitem in vpitem.iter() {
                     if is_cancel(t, tinfo, pitem, &sid) {
                        let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                        cds.push(v);
                    }
                }

            }
        }
    }
    
    return cds;

}



pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    let mut tds:Vec<MakeDecision> = Vec::new();
    for sid in ENGIN_CONF.vsids.iter() {
        
        let e = &ENGIN_CONF.sids[sid];
        let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

        // let tick_size = market.precision.tick_size;
        // let tick_size_keep = count_decimal_places(tick_size) as u32;
        // let rd = RoundingStrategy::ToZero;
        // let ru = RoundingStrategy::AwayFromZero;
        
        let mut contract_value = 1.;
        if e["etype"] == "swap"  && e["exchange"] == "okx" {
            contract_value = market.contract_value.unwrap();
            println!("contract_value={}",contract_value);
        }
    
        let dinfo:&mut DepthInfo = tinfo.depths.get_mut(&sid).unwrap();
        let pos:f64 = *tinfo.pos.get(&sid).unwrap();
        
        if !dinfo.is_finish_snap {
            continue
        }
        let mid:f64 = (dinfo.bid1 + dinfo.ask1) / 2.;
        let amount_hand_token = ARBMTSAMPLING_CONF.amountu / contract_value / mid;


        for range in &ARBMTSAMPLING_CONF.ranges {
            let pairkey = sid.to_string()+":"+&range.to_string();
            let num_bids = get_pending_num_from_key_bid(*sid, &pairkey.to_string());
            if num_bids == 0 {
                let bid_price:f64 = dinfo.bid1 * (1. - *range);
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "bid_m_".to_string()+&pairkey+"_"+ &curr_cid.to_string();

                tds.push(MakeDecision{create_ts:t[0] as i64,client_order_id:client_order_id, max_order_keep_s:0, side:"buy".to_string(), sid:*sid, ttype:"maker".to_string(), price:bid_price, amount:amount_hand_token, from_key:pairkey.to_string(),target_sid:-1});
            }

            let num_asks = get_pending_num_from_key_ask(*sid, &pairkey.to_string());
            if num_asks == 0 {
                let ask_price:f64 = dinfo.ask1 * (1. + *range);
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "ask_m_".to_string()+&pairkey+"_"+ &curr_cid.to_string();

                tds.push(MakeDecision{create_ts:t[0] as i64, client_order_id:client_order_id, max_order_keep_s:0, side:"sell".to_string(), sid:*sid, ttype:"maker".to_string(), price:ask_price, amount:amount_hand_token, from_key:pairkey.to_string(),target_sid:-1});
            }

            
        }
    }
    
    
    
    for sid in ENGIN_CONF.vsids.iter() {
        //每次一个taker
        
        let e = &ENGIN_CONF.sids[sid];
        let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

        // let tick_size = market.precision.tick_size;
        // let tick_size_keep = count_decimal_places(tick_size) as u32;
        // let rd = RoundingStrategy::ToZero;
        // let ru = RoundingStrategy::AwayFromZero;
        
        let mut contract_value = 1.;
        if e["etype"] == "swap"  && e["exchange"] == "okx" {
            contract_value = market.contract_value.unwrap();
            println!("contract_value={}",contract_value);
        }
    
        let dinfo:&mut DepthInfo = tinfo.depths.get_mut(&sid).unwrap();
        let pos:f64 = *tinfo.pos.get(&sid).unwrap();
        
        if !dinfo.is_finish_snap {
            continue
        }
        let mid:f64 = (dinfo.bid1 + dinfo.ask1) / 2.;
        let amount_hand_token = ARBMTSAMPLING_CONF.amountu / contract_value / mid;
        
        let pairkey = "".to_string();
        let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
        let client_order_id = "bid_t_".to_string() + &curr_cid.to_string();
        tds.push(MakeDecision{create_ts:t[0] as i64,
            client_order_id:client_order_id, 
            max_order_keep_s:0, 
            side:"buy".to_string(), 
            sid:*sid, 
            ttype:"taker".to_string(), 
            price:0., 
            amount:amount_hand_token, 
            from_key:pairkey.to_string(),target_sid:-1});
        
        let curr_cid2 = COID_INC.fetch_add(1, Ordering::Relaxed);
        let client_order_id = "ask_t_".to_string() + &curr_cid2.to_string();
        tds.push(MakeDecision{create_ts:t[0] as i64,
            client_order_id:client_order_id, 
            max_order_keep_s:0, 
            side:"sell".to_string(), 
            sid:*sid, 
            ttype:"taker".to_string(), 
            price:0., 
            amount:amount_hand_token, 
            from_key:pairkey.to_string(),target_sid:-1});
    }

    return tds;
}
