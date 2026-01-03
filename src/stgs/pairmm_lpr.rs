use log::{info};
use std::collections::HashMap;
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,PAIRMM_CONF};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::spending::{get_pending_num_from_key, get_pending_num_dup_key_bid, get_pending_num_dup_key_ask, get_pending_num_from_key_bid, get_pending_num_from_key_ask};
use crate::spending::{S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS, PendingItem};
//use ordered_float::OrderedFloat;
use std::time::Duration;
use parking_lot::RwLock;
//use markets::Market;
use crate::symbolinfo::get_market;


use rust_decimal::Decimal;
use std::str::FromStr;
use rust_decimal::prelude::*;
use csv::ReaderBuilder;

#[derive(Debug)]
pub struct CITEM {
    pub sidm:i32,
    pub sidt:i32,
    pub sidet:String,
    pub from_key:String,
    pub total_amount:f64,
    pub curr_amount:f64,
    pub start_close_ts:i64,
}

lazy_static! {
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);
    
     static ref COLS: HashMap<String, usize> = {
        let mut map = HashMap::new();
        map.insert("ts".to_string(), 0);
        map.insert("f_0_1".to_string(), 1);
        map.insert("sf_0_1".to_string(), 2);

        map.insert("f_1_0".to_string(), 3);
        map.insert("sf_1_0".to_string(), 4);
        map
    };

    static ref ONGOING_BID: RwLock<HashMap<String, CITEM>> = {
        let mut r = RwLock::new(HashMap::new());
        r
    };
    static ref ONGOING_ASK: RwLock<HashMap<String, CITEM>> = {
        let mut r = RwLock::new(HashMap::new());
        r
    };
}

pub fn cb_filled(tinfo:&mut TradeInfo, pitem:&PendingItem, filled_amount:f64) {
    info!("cb_filled, fkey={} coid={} amount={}", pitem.from_key, pitem.client_order_id, pitem.amount);
    
    if  pitem.from_key.starts_with("cl_") {
        let ps: Vec<&str> = pitem.from_key.split("cl_").collect();
        let open_from_key = ps[1];
        //find in ongoing
        if pitem.side == "bid" {
            if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
                if ongoing.contains_key(&open_from_key.to_string()) {
                    let mut o = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                    o.curr_amount+=filled_amount;
                    info!("fkey={} coid={} fillamount={} camount={} totalamount={}", open_from_key, pitem.client_order_id, filled_amount, o.curr_amount, o.total_amount);
                    if o.curr_amount >= o.total_amount {
                        info!("ONGOING_BID drop fkey={}", open_from_key);
                        ongoing.remove(&open_from_key.to_string());
                    }
                }
            }
        }
        else if pitem.side == "ask" {
            if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
                if ongoing.contains_key(&open_from_key.to_string()) {
                    let mut o = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                    o.curr_amount+=filled_amount;
                    info!("fkey={} coid={} fillamount={} camount={}, totalamount={}", open_from_key, pitem.client_order_id, filled_amount, o.curr_amount, o.total_amount);
                    if o.curr_amount >= o.total_amount {
                        info!("ONGOING_ASK drop fkey={}", open_from_key);
                        ongoing.remove(&open_from_key.to_string());
                    }
                }
            }            
        }
        else {
            panic!("no such side={}", pitem.side);
        }
        
    }
    else {
        panic!("from_key={} client_order_id={}", pitem.from_key, pitem.client_order_id);
    }
}


pub fn cb_finished(ts:i64, tinfo:&mut TradeInfo, pitem:&PendingItem, is_cancel:bool) {
    info!("cb finished, fkey={} amount={} coid={} pitem.side={} is_cancel={}", pitem.from_key, pitem.amount, pitem.client_order_id, pitem.side, is_cancel);
    info!("cb finished, pitem={:?}", pitem);
    if pitem.client_order_id.starts_with("o") {
        let ps: Vec<&str> = pitem.from_key.split(":").collect();
        let sidt = ps[2].parse::<i32>().unwrap();
        let sidm = ps[1].parse::<i32>().unwrap();
        
        let e_m = &ENGIN_CONF.sids[&sidm];
        let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
        let dinfo_m:&DepthInfo = tinfo.depths.get(&sidm).unwrap();
        let pos_m:f64 = *tinfo.pos.get(&sidm).unwrap();
        
        let mut contract_value_m = 1.;
        if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
            contract_value_m = market_m.contract_value.unwrap();
        }
        
        if pitem.side == "bid" {
            //ask close
            let amount_deal = ((pitem.amount_init - pitem.amount) * contract_value_m);
            if amount_deal != 0. {     

                let citem = CITEM{sidm:sidm,sidt:sidt, sidet:"ask".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, curr_amount:0., start_close_ts:ts};
                info!("cb finish , put {}", pitem.from_key.to_string());
                if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
                    ongoing.insert(pitem.from_key.to_string(), citem);
                }
            }
        }
        else if pitem.side == "ask" {
            let amount_deal = ((pitem.amount_init - pitem.amount) * contract_value_m);
            if amount_deal != 0. {
                
            
                let citem = CITEM{sidm:sidm, sidt:sidt, sidet:"bid".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, curr_amount:0., start_close_ts:ts};
                if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
                    ongoing.insert(pitem.from_key.to_string(), citem);
                }
            }
        } 
        else {
            panic!("wrong side");
        }
    }
}

pub fn get_ongoing_num(sidt:i32, side:&str)->i32 {
    let mut num = 0;
    if side == "bid" {
        let ongoing = ONGOING_BID.read();
        info!("ongoing bid={:?}", ongoing);
        for (k,ci) in ongoing.iter() {
            if ci.sidt == sidt {
                num+=1;
            }
        }
    } else if side == "ask" {
        let ongoing = ONGOING_ASK.read();
        info!("ongoing ask={:?}", ongoing);
        for (k,ci) in ongoing.iter() {
            if ci.sidt == sidt {
                num+=1;
            }
        }
    }
    
    return num;
}

fn linear_interpolation(x1: f64, y1: f64, x2: f64, y2: f64, x_interp: f64) -> f64 {
    if x_interp < x1 {
        return y1;
    }
    if x_interp > x2 {
        return y2;
    }
    
    let delta_x = x2 - x1;
    let delta_y = y2 - y1;
    let delta_x_interp = x_interp - x1;
    
    let slope = delta_y / delta_x;
    let y_interp = y1 + slope * delta_x_interp;
    
    y_interp
}

pub fn is_cancel(t:&Vec<f64>, tinfo:&mut TradeInfo, pitem:&PendingItem, sid:&i32) -> bool{
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
    
    if (t[0]*1000.) as i64 - pitem.create_ts > (PAIRMM_CONF.max_order_keep_s*1000) as i64 {
        info!("pending last too long cid={} o={} c={}", pitem.client_order_id, t[0] as i64, pitem.create_ts);
        return true
    }
    let e = &ENGIN_CONF.sids[sid];
    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    
    //计算信号
    if pitem.client_order_id.starts_with("o") {
        let ps: Vec<&str> = pitem.from_key.split(":").collect();
        //info!("from={}", pitem.from_key);
        let pair = ps[1].to_string() + "_" + &ps[2].to_string();
        let key = "sf_".to_string() + &pair;
        let f = t[*COLS.get(&key).unwrap()] as f64;
        if pitem.side == "bid" {
            if  -f < 0.0003{
                return true;
            }
        }
        else {
            if f  < 0.0003{
                return true;
            }
        }        
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

pub fn get_target_prem_by_ranges(ranges:Vec<f64>, f:f64, sf:f64, is_open:bool) ->Vec<f64>{
    let mut vs:Vec<f64> = Vec::new();
    if f > 0. {
        return vs;
    }    
    for range in ranges {
        if is_open && range < 0.0003 {
            continue;
        }        
        info!("f={}, range={}", f, range);
        if -sf > 0.0003{
            vs.push(range);
        }
    }
    return vs;
}

pub fn get_range_of(ts:i64, side:&str, sid:i32)->Vec<f64> {
    let mut vs:Vec<f64> = vec![0.0002, 0.0004, 0.0006, 0.0008, 0.0015];
    return vs;
}

pub fn get_rbs_from_df_maker_close(t:&Vec<f64>, df:f64,  side:&str) ->f64 {
    let mut rs = -1.;
    if df > 0. {
        return rs;
    }
    let target_thr_maker_close = 0.0004;    
    if side == "buy" {        
        let rbs:Vec<f64> = vec![0.0002,0.0004,0.0006,0.0008, 0.001];
        for rb in rbs {
            
            let prem = rb + (-df);
            if prem > target_thr_maker_close{
                rs = rb;
                break;
            }
        }
        return rs;
    } else if side == "sell" {
        let rbs:Vec<f64> = vec![0.0002,0.0004,0.0006,0.0008,0.001];
        for rb in rbs {
            let prem = rb + (-df);
            if prem > target_thr_maker_close{
                rs = rb;
                break
            }
        }
        return rs;
    } else {
        panic!("no side={}", side);
    }
    return rs   
}

pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    let mut tds:Vec<MakeDecision> = Vec::new();
    let max_pos_u = PAIRMM_CONF.amountu * 500.;
    let max_pending_pair = 1;
    let max_ongoing = 5;
    let ts = t[0];

    // close
    for sid in ENGIN_CONF.vsids.iter() {
        //0-1, 1-0
        let e_m = &ENGIN_CONF.sids[sid];
        let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
        let dinfo_m:&DepthInfo = tinfo.depths.get(&sid).unwrap();
        let pos_m:f64 = *tinfo.pos.get(&sid).unwrap();
                
        let mut contract_value_m = 1.;
        if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
            contract_value_m = market_m.contract_value.unwrap();
        }
        
        let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;
        let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
        
        if !dinfo_m.is_finish_snap || mid_m == 0. {
            continue
        }
        
        let last_y:f64 = 60.;
        let last_rb_start:f64 = 0.003;
        let last_rb_end:f64 = -0.003;
                
        let ongoing = ONGOING_BID.read();
        for (k,ci) in ongoing.iter() {
            if ci.sidt != *sid {
                continue;
            }
            
            let nbids = get_pending_num_from_key_bid(*sid, &ci.from_key);
            if nbids > 0 {
                continue;
            }
            
            let curr_last = (t[0] as i32 - (ci.start_close_ts /1000) as i32) as f64;
            let range_delta = linear_interpolation(0., last_rb_start, last_y , last_rb_end, curr_last);
            
            let amount = (ci.total_amount - ci.curr_amount) / contract_value_m;
            info!("ts={} fkey={} total_amount={} curr_amount={} start_close={} last={} amount={}", ts, ci.from_key, ci.total_amount, ci.curr_amount, ci.start_close_ts, curr_last, amount);
           
            let key = "sf_".to_string() + &ci.sidt.to_string() + "_" + &ci.sidm.to_string();
            let sf = t[*COLS.get(&key).unwrap()] as f64;
            
            let rb_close = get_rbs_from_df_maker_close(t, sf, "buy");
            let mut rb = rb_close + range_delta;
            rb = rb.max(0.);
            if rb_close == -1. {
                continue;
            }
            
            let e_t = &ENGIN_CONF.sids[&ci.sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&ci.sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&ci.sidt).unwrap();

            let pkey = "cl_".to_string() + &ci.from_key;
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "cbid_m_".to_string()+&ci.sidm.to_string()+"_"+&ci.sidt.to_string()+"_"+&rb_close.to_string()+"_"+ &curr_cid.to_string();
            let nbids = get_pending_num_from_key_bid(*sid, &pkey);
            if nbids > 0 {
                continue;
            }
            let bid_price:f64 = dinfo_t.bid1 * (1. - rb);
            tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:PAIRMM_CONF.max_order_keep_s as i32, 
                side:"buy".to_string(), 
                sid:*sid, 
                ttype:"maker".to_string(), 
                price:bid_price, 
                amount:amount, 
                from_key:pkey.to_string(),
                dup_key:"".to_string(),
                target_sid:-1
            });
        }
            
        let ongoing = ONGOING_ASK.read();
        for (k,ci) in ongoing.iter() {
            if ci.sidt != *sid {
                continue;
            }
            
            let nasks = get_pending_num_from_key_ask(*sid, &ci.from_key);
            if nasks > 0 {
                continue;
            }
            let curr_last = (t[0] as i32 - (ci.start_close_ts /1000) as i32) as f64;
            let range_delta = linear_interpolation(0., last_rb_start, last_y , last_rb_end, curr_last);
            
            let amount = (ci.total_amount - ci.curr_amount) / contract_value_m;
            info!("ts={}  fkey={} total_amount={} curr_amount={} start_close={} last={} amount={}", ts, ci.from_key, ci.total_amount, ci.curr_amount, ci.start_close_ts, curr_last, amount);

            let key = "sf_".to_string() + &ci.sidt.to_string() + "_" + &ci.sidm.to_string();
            let sf = t[*COLS.get(&key).unwrap()] as f64;
       
            let rb_close = get_rbs_from_df_maker_close(t, -sf, "sell");
            let mut rb = rb_close + range_delta;
            rb = rb.max(0.);
            if rb_close == -1. {
                continue;
            }
            let e_t = &ENGIN_CONF.sids[&ci.sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&ci.sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&ci.sidt).unwrap();
            
            let pkey = "cl_".to_string() + &ci.from_key;
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "cask_m_".to_string()+&ci.sidm.to_string()+"_"+&ci.sidt.to_string()+"_"+&rb_close.to_string()+"_"+ &curr_cid.to_string();
            let nasks = get_pending_num_from_key_ask(*sid, &pkey);
            if nasks > 0 {
                continue;
            }
            let ask_price:f64 = dinfo_t.ask1 * (1. + rb);
            info!("f ask_price={} [{} {}]", ask_price, dinfo_t.bid1, dinfo_t.ask1);
            tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:PAIRMM_CONF.max_order_keep_s as i32, 
                side:"sell".to_string(), 
                sid:*sid, 
                ttype:"maker".to_string(), 
                price:ask_price, 
                amount:amount, 
                from_key:pkey.to_string(),
                dup_key:"".to_string(),
                target_sid:-1
            });      
        }
    }
    
    //open
    for sid in ENGIN_CONF.vsids.iter() {
        //0-1, 1-0
        let e_m = &ENGIN_CONF.sids[sid];
        let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
        let dinfo_m:&DepthInfo = tinfo.depths.get(&sid).unwrap();
        let pos_m:f64 = *tinfo.pos.get(&sid).unwrap();
        
        let mut contract_value_m = 1.;
        if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
            contract_value_m = market_m.contract_value.unwrap();
        }
        
        let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;
        let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
        let upos_m:f64 = pos_m * contract_value_m * mid_m;
        let gopenu = tinfo.open * mid_m;
        info!("gopen,{},{}", t[0], gopenu);
        if !dinfo_m.is_finish_snap || mid_m == 0. {
            continue
        }
        
        //bid
        for sidt in ENGIN_CONF.vsids.iter() {
            if sid == sidt {
                continue;
            }

            //0-1, 1-0
            let e_t = &ENGIN_CONF.sids[sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&sidt).unwrap();
            let mut contract_value_t = 1.;
            if e_t["etype"] == "swap"  && e_t["exchange"] == "okx" {
                contract_value_t = market_t.contract_value.unwrap();
            }

            let mid_t:f64 = (dinfo_t.bid1 + dinfo_t.ask1) / 2.;
            if !dinfo_t.is_finish_snap  || mid_t == 0. {
                continue
            }

            let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
            info!("sidt={} amt={} cv={}", sidt, amount_hand_token, contract_value_t);
            let upos_t:f64 = pos_t * contract_value_t * mid_t;
            
            let key = "f_".to_string() + &sid.to_string() + "_" + &sidt.to_string();
            let skey = "sf_".to_string() + &sid.to_string() + "_" + &sidt.to_string();

            let f = t[*COLS.get(&key).unwrap()] as f64;
            let sf = t[*COLS.get(&skey).unwrap()] as f64;
            info!("ts={} key={} f={}", t[0], key, f);
            
            let ranges_bids = get_range_of(t[0] as i64, "bid", *sid);
            let ranges_asks = get_range_of(t[0] as i64, "ask", *sid);
            info!("sid={} ranges_bids={:?} ranges_asks={:?}", sid, ranges_bids, ranges_asks);
            
            let mut is_open = false;
            if upos_m > 0. {
                is_open = true;
            }
            
            let ranges_bid_prem = get_target_prem_by_ranges(ranges_bids, f, sf, is_open);
            info!("sid={} ranges_bid_prem={:?}", sid, ranges_bid_prem);
            
            let pps = PAIRPOS.read();
            let key = sid.to_string() + ":" + &sidt.to_string();
            let pairpos = pps.get(&key).unwrap();
            let pairposu = *pairpos * mid_m;
            
            //bid
            if upos_m > max_pos_u || upos_t < -max_pos_u {
                info!("bid sid=[{} {}] pos exeeds max, uposm={} upost={}", sid, sidt, upos_m, upos_t);
                continue;
            }
            
            if true {
                let num_ongoing = get_ongoing_num(*sidt, "ask");
                info!("num_ongoing ask={}", num_ongoing);
                if num_ongoing > max_ongoing {
                    info!("onoing={} exeeds max, ignore open", num_ongoing);
                    continue
                }
                
                for range in ranges_bid_prem {
                    let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
                    let fkey = "bid:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string()+":"+&ts.to_string();
                    let dupkey = "bid:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string();
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "obid".to_string()+":"+&sid.to_string()+":"+&sidt.to_string()+"_"+&range.to_string()+"_"+ &curr_cid.to_string();
                    
                    let nbids = get_pending_num_dup_key_bid(*sid, &dupkey);
                    if nbids >= max_pending_pair {
                        info!("bid ignore dup={}", dupkey);
                        continue;
                    }

                    let bid_price:f64 = dinfo_m.bid1 * (1. - range);
                    tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                        client_order_id:client_order_id, 
                        max_order_keep_s:PAIRMM_CONF.max_order_keep_s as i32, 
                        side:"buy".to_string(), 
                        sid:*sid, 
                        ttype:"maker".to_string(), 
                        price:bid_price, 
                        amount:amount_hand_token, 
                        from_key:fkey.to_string(),
                        dup_key:dupkey.to_string(),
                        target_sid:-1
                    });
                }
            }
        }
        
        //ask
        for sidt in ENGIN_CONF.vsids.iter() {
            if sid == sidt {
                continue;
            }

            let e_t = &ENGIN_CONF.sids[sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&sidt).unwrap();

            let mut contract_value_t = 1.;
            if e_t["etype"] == "swap"  && e_t["exchange"] == "okx" {
                contract_value_t = market_t.contract_value.unwrap();
            }           

            let mid_t:f64 = (dinfo_t.bid1 + dinfo_t.ask1) / 2.;
            if !dinfo_t.is_finish_snap  || mid_t == 0. {
                continue
            }
            let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
            let upos_t:f64 = pos_t * contract_value_t * mid_t;
            info!("sidt={} amt={} cv={}", sidt, amount_hand_token, contract_value_t);
            
            let key = "f_".to_string() + &sid.to_string() + "_" + &sidt.to_string();
            let skey = "sf_".to_string() + &sid.to_string() + "_" + &sidt.to_string();
            let f = t[*COLS.get(&key).unwrap()] as f64;
            let sf = t[*COLS.get(&skey).unwrap()] as f64;
            info!("ts={} key={} f={}", t[0], key, f);
            
            let ranges_bids = get_range_of(t[0] as i64, "bid", *sid);
            let ranges_asks = get_range_of(t[0] as i64, "ask", *sid);
            info!("sid={} ranges_bids={:?} ranges_asks={:?}", sid, ranges_bids, ranges_asks);
            
            let mut is_open = false;
            if upos_m < 0. {
                is_open = true;
            }
            
            let ranges_ask_prem = get_target_prem_by_ranges(ranges_asks, -f, -sf, is_open);
            info!("sid={} ranges_ask_prem={:?}", sid, ranges_ask_prem);
            
            let pps = PAIRPOS.read();
            let key = sid.to_string() + ":" + &sidt.to_string();
            let pairpos = pps.get(&key).unwrap();
            let pairposu = *pairpos * mid_m;
            
            if upos_m < -max_pos_u || upos_t > max_pos_u {
                info!("ask sid=[{} {}] pos exeeds max, uposm={} upost={}", sid, sidt, upos_m, upos_t);
                continue;
            }
            
            if true {
                let num_ongoing = get_ongoing_num(*sidt, "bid");
                info!("num_ongoing bid={}", num_ongoing);
                if num_ongoing > max_ongoing {
                    info!("onoing={} exeeds max, ignore open", num_ongoing);
                    continue
                }
                
                for range in ranges_ask_prem {
                    let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
                    let fkey = "ask:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string() + ":" +&ts.to_string();
                    let dupkey = "ask:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string();
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "oask".to_string()+":"+&sid.to_string()+":"+&sidt.to_string()+"_"+&range.to_string()+"_"+ &curr_cid.to_string();
                    
                    let nasks = get_pending_num_dup_key_ask(*sid, &dupkey);
                    info!("range={} nasks={}", range, nasks);
                    if nasks >= max_pending_pair {
                        info!("ask ignore dup={}", dupkey);
                        continue;
                    }

                    let ask_price:f64 = dinfo_m.ask1 * (1. + range);
                    tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                        client_order_id:client_order_id, 
                        max_order_keep_s:PAIRMM_CONF.max_order_keep_s as i32, 
                        side:"sell".to_string(), 
                        sid:*sid, 
                        ttype:"maker".to_string(), 
                        price:ask_price, 
                        amount:amount_hand_token, 
                        from_key:fkey.to_string(),
                        dup_key:dupkey.to_string(),
                        target_sid:-1
                    });
                }
            }
        }
    }
    return tds;
}
