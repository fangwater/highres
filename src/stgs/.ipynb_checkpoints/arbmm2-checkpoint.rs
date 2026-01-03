use log::{info};
use std::collections::HashMap;
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,ARBMM2_CONF};
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
    static ref COLS: HashMap<String, usize> = {
        let mut map = HashMap::new();
        map.insert("ts".to_string(), 0);
        map.insert("f_0".to_string(), 1);
        map.insert("s_0".to_string(), 2);
        map.insert("f_1".to_string(), 3);
        map.insert("s_1".to_string(), 4);
        map.insert("f_2".to_string(), 5);
        map.insert("s_2".to_string(), 6);
        map.insert("f_3".to_string(), 7);
        map.insert("s_3".to_string(), 8);
        map.insert("f_4".to_string(), 9);
        map.insert("s_4".to_string(), 10);

        map
    };
    
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);

    
}


fn linear_interpolation(x1: f64, y1: f64, x2: f64, y2: f64, x_interp: f64) -> f64 {
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    let delta_x_interp = x_interp - x1;
    
    let slope = delta_y / delta_x;
    let y_interp = y1 + slope * delta_x_interp;
    
    y_interp
}

fn count_decimal_places(num: f64) -> usize {
    let num_str = num.to_string();
    if let Some(decimal_index) = num_str.find('.') {
        num_str.len() - decimal_index - 1
    } else {
        0
    }
}

pub fn get_rbs_from_df(df:f64,  s:f64) ->Vec<f64> {
    let mut rs:Vec<f64> = Vec::new();
    
    //简单的线性逻辑
    let thr:f64 = 0.0003;
  
    
    let rbs:Vec<f64> = vec![0.0002, 0.0004, 0.0006];
    if df > 0. {
        return rs;
    }
    
    for rb in rbs {
        let prem = rb + (-df);
        if prem > thr {
            rs.push(rb);
        }
    }
    
    return rs;
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
        let upos:f64 = pos * contract_value * mid;
        
        let gopenu = tinfo.open * mid;
        
        let gopen_ctl_max = ARBMM2_CONF.amountu * 1000.;
        let full_pos = 200. * ARBMM2_CONF.amountu;
        let rb_bid_openrisk = linear_interpolation(-gopen_ctl_max, -0.005, gopen_ctl_max, 0.005, gopenu);
        let rb_ask_openrisk = linear_interpolation(-gopen_ctl_max, 0.005, gopen_ctl_max, -0.005, gopenu);
        
        let rb_bid_posctl = linear_interpolation(-full_pos as f64, -0.001, full_pos as f64, 0.001, upos);
        let rb_ask_posctl = linear_interpolation(-full_pos as f64, 0.001, full_pos as f64, -0.001, upos);
        let thr_taker = 0.001;
        let amount_hand_token = ARBMM2_CONF.amountu / contract_value / mid;
        let gopen_ctl_amount_max_u = ARBMM2_CONF.amountu * 30.;
        
        let amount_delta_bid_u = linear_interpolation(-gopen_ctl_amount_max_u as f64, 2.*ARBMM2_CONF.amountu, gopen_ctl_amount_max_u as f64, 0., gopenu);
        let amount_delta_ask_u = linear_interpolation(-gopen_ctl_amount_max_u as f64, 0., gopen_ctl_amount_max_u as f64, 2.*ARBMM2_CONF.amountu, gopenu);
        let amount_delta_bid_token =  amount_delta_bid_u / contract_value / mid;
        let amount_delta_ask_token =  amount_delta_ask_u / contract_value / mid;
        
        let df = t[*COLS.get(&("f_".to_string()+&sid.to_string())).unwrap()];
        let fstd = t[*COLS.get(&("s_".to_string()+&sid.to_string())).unwrap()];
        if *sid == 3 {
            info!("hh1, df={}", df);
        }
        
        let rb_bid_dfs = get_rbs_from_df(df, fstd);
        let rb_ask_dfs = get_rbs_from_df(-df, fstd);
        
        
        
        
        for rb_bid_df in rb_bid_dfs {
            
            if *sid == 3 {
                info!("hh, rb_bid_df={}", rb_bid_df);
            }
            
            let mut rb_bid = rb_bid_df + rb_bid_openrisk + rb_bid_posctl;
            rb_bid = rb_bid.max(0.);
            if rb_bid > 0.0005 {
                continue;
            }
            
            let pkey = "bid:".to_string()+&rb_bid_df.to_string();
            let nbids = get_pending_num_from_key_bid(*sid, &pkey);
            
            if nbids > 0 {
                continue;
            }
            
            let amount_token = amount_hand_token + amount_delta_bid_token;
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "bid_m_".to_string()+&sid.to_string()+"_"+&rb_bid_df.to_string()+"_"+ &curr_cid.to_string();
            
            let bid_price:f64 = dinfo.bid1 * (1. - rb_bid);
            tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:ARBMM2_CONF.max_order_keep_s, 
                side:"buy".to_string(), 
                sid:*sid, 
                ttype:"maker".to_string(), 
                price:bid_price, 
                amount:amount_token, 
                from_key:pkey.to_string(),target_sid:-1
            });
        }
        
        for rb_ask_df in rb_ask_dfs {
            let mut rb_ask = rb_ask_df + rb_ask_openrisk + rb_ask_posctl;
            rb_ask = rb_ask.max(0.);
            if rb_ask > 0.0005 {
                continue;
            }
            
            let pkey = "ask:".to_string()+&rb_ask_df.to_string();
            let nasks = get_pending_num_from_key_ask(*sid, &pkey);
            
            if nasks > 0 {
                continue;
            }
            
            let amount_token = amount_hand_token + amount_delta_ask_token;
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "ask_m_".to_string()+&sid.to_string()+"_"+&rb_ask_df.to_string()+"_"+ &curr_cid.to_string();
            
            let ask_price:f64 = dinfo.ask1 * (1. + rb_ask);
            tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:ARBMM2_CONF.max_order_keep_s, 
                side:"sell".to_string(), 
                sid:*sid, 
                ttype:"maker".to_string(), 
                price:ask_price, 
                amount:amount_token, 
                from_key:pkey.to_string(), target_sid:-1
                
            });
        }

    }

    return tds;
}


pub fn is_cancel(t:&Vec<f64>, tinfo:&mut TradeInfo, pitem:&PendingItem, sid:&i32) -> bool{
    //计算信号
    let dinfo:&DepthInfo = tinfo.depths.get(sid).unwrap();
    let pos:f64 = *tinfo.pos.get(sid).unwrap();
    if !dinfo.is_finish_snap {
        info!("pitem coid={} rebuild, cancel",pitem.client_order_id);
        return true;
    }
    
    let e = &ENGIN_CONF.sids[sid];
    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    let mut contract_value = 1.;
    if e["etype"] == "swap"  && e["exchange"] == "okx" {
        contract_value = market.contract_value.unwrap();
        println!("contract_value={}",contract_value);
    }
    
    let mid:f64 = (dinfo.bid1 + dinfo.ask1) / 2.;
    let upos:f64 = pos * mid * contract_value;
    
    if (t[0]*1000.) as i64 - pitem.create_ts > (ARBMM2_CONF.max_order_keep_s*1000) as i64 {
        info!("pending last too long o={} c={}", t[0] as i64, pitem.create_ts);
        return true
    }

    let gopenu = tinfo.open * mid;
    let gopen_ctl_max = ARBMM2_CONF.amountu * 1000.;
    let full_pos = 200. * ARBMM2_CONF.amountu;
    let rb_bid_openrisk = linear_interpolation(-gopen_ctl_max, -0.005, gopen_ctl_max, 0.005, gopenu);
    let rb_ask_openrisk = linear_interpolation(-gopen_ctl_max, 0.005, gopen_ctl_max, -0.005, gopenu);

    let rb_bid_posctl = linear_interpolation(-full_pos as f64, -0.0005, full_pos as f64, 0.0005, upos);
    let rb_ask_posctl = linear_interpolation(-full_pos as f64, 0.0005, full_pos as f64, -0.0005, upos);
    
    
    let _thr_taker = 0.001;
    

    let e = &ENGIN_CONF.sids[sid];
    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    
    let tick_size = market.precision.tick_size;
    let tick_size_keep = count_decimal_places(tick_size) as u32;
    let rd = RoundingStrategy::ToZero;
    let ru = RoundingStrategy::AwayFromZero;

    let pairkey = pitem.from_key.to_string().replace(":", "_");
    let df = t[*COLS.get(&("f_".to_string()+&sid.to_string())).unwrap()];
    let fstd = t[*COLS.get(&("s_".to_string()+&sid.to_string())).unwrap()];
    
    //get df
    if pitem.side == "bid" {
        //计算如果还在溢价区间内， 则不动, 
        //TODO 这里还需要考虑last时间 
        
        let curr_rb = (dinfo.bid1 - pitem.price) / dinfo.bid1;
        let prem = curr_rb + (-df);
        if prem < 0.0004 {
            return true;
        }

    } else if pitem.side == "ask" {
        let curr_rb = (pitem.price - dinfo.ask1) / dinfo.ask1;
        let prem = curr_rb + df;
        if prem < 0.0004 {
            return true;
        }
    
    } else {
        panic!("side={}", pitem.side);
    }
    return false
}


pub fn cancel(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<CancelDecision>{
    let mut cds:Vec<CancelDecision> = Vec::new();
    let msts = (t[0] * 1000.) as i64;
    // if msts % 2000 == 0 {
    //     return cds
    // }
    
    
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

