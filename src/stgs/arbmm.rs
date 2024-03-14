use log::{info};
use std::collections::HashMap;
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,ARBMM_CONF};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::spending::{get_pending_num_from_key};
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
        map.insert("fdf_0_1".to_string(), 1);
        map.insert("fdf_0_2".to_string(), 2);
        map.insert("fdf_1_0".to_string(), 3);
        map.insert("fdf_1_2".to_string(), 4);
        map.insert("fdf_2_0".to_string(), 5);
        map.insert("fdf_2_1".to_string(), 6);
        map.insert("fdf_mean".to_string(), 7);
        map.insert("fdf_std".to_string(), 8);
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

pub fn get_f_from_df(df:f64, m:f64, s:f64) ->f64 {
    let thr0:f64 = m;
    let thr1:f64 = m + 0.3 * s;
    let thr2:f64 = m + 0.5 * s; 
    
    let rb0 = 0.0005;
    let rb1 = 0.0002;
    let rb2 = 0.0001;
    
    if df.abs() > 0.001 {
        return -1.
    }
        
    if df < -thr2 {
        return rb2
    }
    if df < -thr1 {
        return rb1
    }
    if df < -thr0 {
        return rb0
    }
    return -1.
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
    
    if (t[0]*1000.) as i64 - pitem.create_ts > (ARBMM_CONF.max_order_keep_s*1000) as i64 {
        info!("pending last too long o={} c={}", t[0] as i64, pitem.create_ts);
        return true
    }

    let gopenu = tinfo.open * mid;
    let gopen_ctl_max = ARBMM_CONF.amountu * 1000.;
    let full_pos = 200. * ARBMM_CONF.amountu;
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
 
    
    let fkey = "fdf_".to_string()+&pairkey;
    println!("fkey={}", fkey);
    let df = t[*COLS.get(&fkey).unwrap()];
    let fstd = t[*COLS.get(&"fdf_std".to_string()).unwrap()];
    let fmean = t[*COLS.get(&"fdf_mean".to_string()).unwrap()];
    
    //get df
    if pitem.side == "bid" {
        let rb_bid_df = get_f_from_df(df, fmean, fstd);
        if rb_bid_df == -1. {
            info!("pitem coid={} sig none, cancel",pitem.client_order_id);
            return true;
        }
        let mut rb_bid = rb_bid_df + rb_bid_openrisk + rb_bid_posctl;
        rb_bid = rb_bid.max(0.);
        if rb_bid > 0.001 {
            info!("pitem coid={} factor rb_bid = {} > 0.001, cancel", pitem.client_order_id, rb_bid);
            return true;
        }
        
        let bid_price:f64 = dinfo.bid1 * (1. - rb_bid);
        
        let price_adj:f64 = adjust_price(bid_price, tick_size, tick_size_keep, true);
        let pdec_bid = Decimal::from_str(&price_adj.to_string()).unwrap();
        let bid_price_f = pdec_bid.round_dp_with_strategy(tick_size_keep, rd);
        let bid_price_f64: f64 = bid_price_f.to_f64().unwrap();
        
        if pitem.price > bid_price_f64 {
            info!("pitem coid={} pending.price = {} > bid_price_f64 = {}, cancel", pitem.client_order_id, pitem.price, bid_price_f64);
            return true;
        }
    } else if pitem.side == "ask" {
        let rb_ask_df = get_f_from_df(-df, fmean, fstd);
        if rb_ask_df == -1. {
            info!("pitem coid={} sig none, cancel",pitem.client_order_id);
            return true;
        }
        let mut rb_ask = rb_ask_df + rb_ask_openrisk + rb_ask_posctl;
        rb_ask = rb_ask.max(0.);
        if rb_ask > 0.001 {
            info!("pitem coid={} factor rb_ask = {} > 0.001, cancel", pitem.client_order_id, rb_ask);
            return true;
        }
        let ask_price:f64 = dinfo.ask1 * (1. + rb_ask);
        let price_adj:f64 = adjust_price(ask_price, tick_size, tick_size_keep, false);
        let pdec_ask = Decimal::from_str(&price_adj.to_string()).unwrap();
        let ask_price_f = pdec_ask.round_dp_with_strategy(tick_size_keep, ru);
        let ask_price_f64: f64 = ask_price_f.to_f64().unwrap();
        
        if pitem.price < ask_price_f64 {
            info!("pitem coid={} pending.price = {} < ask_price_f64 = {}, cancel", pitem.client_order_id, pitem.price, ask_price_f64);
            return true
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
        
        let gopen_ctl_max = ARBMM_CONF.amountu * 1000.;
        let full_pos = 200. * ARBMM_CONF.amountu;
        let rb_bid_openrisk = linear_interpolation(-gopen_ctl_max, -0.005, gopen_ctl_max, 0.005, gopenu);
        let rb_ask_openrisk = linear_interpolation(-gopen_ctl_max, 0.005, gopen_ctl_max, -0.005, gopenu);
        
        let rb_bid_posctl = linear_interpolation(-full_pos as f64, -0.0005, full_pos as f64, 0.0005, upos);
        let rb_ask_posctl = linear_interpolation(-full_pos as f64, 0.0005, full_pos as f64, -0.0005, upos);
        let thr_taker = 0.001;
        let amount_hand_token = ARBMM_CONF.amountu / contract_value / mid;
        
        
        //pair
        for osid in ENGIN_CONF.vsids.iter() {
            if osid == sid {
                continue;
            }
            
            let pairkey = sid.to_string()+":"+&osid.to_string();
            let fkey = "fdf_".to_string()+&sid.to_string()+"_"+&osid.to_string();
            let df = t[*COLS.get(&fkey).unwrap()];
            let fstd = t[*COLS.get(&"fdf_std".to_string()).unwrap()];
            let fmean = t[*COLS.get(&"fdf_mean".to_string()).unwrap()];
            
            
            //TODO taker
            
            let rb_bid_df = get_f_from_df(df, fmean, fstd);
            let rb_ask_df = get_f_from_df(-df, fmean, fstd);
            
            let mut rb_bid = rb_bid_df + rb_bid_openrisk + rb_bid_posctl;
            rb_bid = rb_bid.max(0.);
            let mut rb_ask = rb_ask_df + rb_ask_openrisk + rb_ask_posctl;
            rb_ask = rb_ask.max(0.);
            
            let num_bids:i32;
            let num_asks:i32;
            let m = get_pending_num_from_key(*sid, &pairkey);
            match m {
                (_num_bids, _num_asks) => {
                    num_bids = _num_bids;
                    num_asks = _num_asks;
                    info!("pairkey={} num_bids={} num_asks={}", pairkey, num_bids, num_asks);
                }
            }
            
            info!("pairkey={} f={} rb_bid={} rb_ask={} fmean={} fstd={}", pairkey, df, rb_bid, rb_ask, fmean, fstd);
            info!("sid={} openu={} df={} upos={}", sid, gopenu, df, upos);
            if gopenu < 0. && df != -1. && df < -thr_taker && upos < full_pos {
            //if df != -1. && df < -thr_taker && upos < full_pos {

                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "bid_t_".to_string()+&sid.to_string()+"_"+ &curr_cid.to_string();
                
                tds.push(MakeDecision{
                    create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                    client_order_id:client_order_id, 
                    max_order_keep_s:0, side:"buy".to_string(), 
                    sid:*sid, 
                    ttype:"taker".to_string(), 
                    price:0., 
                    amount:amount_hand_token, 
                    from_key:pairkey.to_string()}); 
            }
            if gopenu > 0. && df != -1. && df > thr_taker && upos > -full_pos {
            //if df != -1. && df > thr_taker && upos > -full_pos {
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "ask_t_".to_string()+&sid.to_string()+"_"+ &curr_cid.to_string();
                
                tds.push(MakeDecision{
                    create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                    client_order_id:client_order_id, 
                    max_order_keep_s:0, 
                    side:"sell".to_string(), 
                    sid:*sid, 
                    ttype:"taker".to_string(), 
                    price:0., 
                    amount:amount_hand_token, 
                    from_key:pairkey.to_string()});
            }
            
            let bid_price:f64 = dinfo.bid1 * (1. - rb_bid);
            if rb_bid_df != -1. && df != -1. && num_bids < 2 && rb_bid < 0.0007 {
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "bid_m_".to_string()+&sid.to_string()+"_"+ &curr_cid.to_string();
                
                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,client_order_id:client_order_id, max_order_keep_s:ARBMM_CONF.max_order_keep_s, side:"buy".to_string(), sid:*sid, ttype:"maker".to_string(), price:bid_price, amount:amount_hand_token, from_key:pairkey.to_string()});
            }
            let ask_price:f64 = dinfo.ask1 * (1. + rb_ask);
            if rb_ask_df != -1. && df != -1. && num_asks < 2 && rb_ask < 0.0007 {
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "ask_m_".to_string()+&sid.to_string()+"_"+ &curr_cid.to_string();
                
                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64, client_order_id:client_order_id, max_order_keep_s:ARBMM_CONF.max_order_keep_s, side:"sell".to_string(), sid:*sid, ttype:"maker".to_string(), price:ask_price, amount:amount_hand_token, from_key:pairkey.to_string()});
            }            
  
        }
    }

    return tds;
}
