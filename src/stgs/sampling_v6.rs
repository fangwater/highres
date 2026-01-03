use log::{info};
use std::collections::HashMap;
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,PAIRMM_CONF};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::spending::{get_pending_num_from_sid,get_pending_num_from_key, get_pending_num_dup_key_bid, get_pending_num_dup_key_ask, get_pending_num_from_key_bid, get_pending_num_from_key_ask};
use crate::spending::{S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS, PendingItem};
//use ordered_float::OrderedFloat;
use std::time::Duration;
use parking_lot::RwLock;
//use markets::Market;
use crate::symbolinfo::get_market;
//v.

use rust_decimal::Decimal;
use std::str::FromStr;
use rust_decimal::prelude::*;
use csv::ReaderBuilder;
use ordered_float::OrderedFloat;

#[derive(Debug)]
pub struct CITEM {
    //pub sidc:i32,
    pub sidc:i32,
    pub curr_amount:f64
}

#[derive(Debug)]
pub struct VCITEM {
    //pub sidc:i32,
    pub sidm:i32,
    pub sidt:i32,
    pub sidet:String,
    pub from_key:String,
    pub total_amount:f64,
    pub start_close_ts:i64,
    pub citems:Vec<CITEM>
}

#[derive(Debug)]
pub struct SAMPLEITEM {
    pub lastts:i64,
    pub fkey:String,
    pub sid:i32,
    pub sidt:i32,
    pub side:String,
    pub amount:f64,
    
}

lazy_static! {
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);
    
     static ref COLS: HashMap<String, usize> = {
        let mut map = HashMap::new();
        map.insert("ts".to_string(), 0);
        map.insert("signal1".to_string(), 1);
        map.insert("signal2".to_string(), 2);
        map.insert("signal3".to_string(), 3);
        map.insert("buy_cancel".to_string(), 4);
        map.insert("sell_cancel".to_string(), 5);
        map
    };
    

        
    //list of all ongoing orders
    static ref ONGOING_BID: RwLock<HashMap<String,VCITEM>> = {
        let mut r = RwLock::new(HashMap::new());
        r
    };
    static ref ONGOING_ASK: RwLock<HashMap<String, VCITEM>> = {
        let mut r = RwLock::new(HashMap::new());
        r
    };
    
    static ref SAMPLEOPEN: RwLock<HashMap<String, Vec<SAMPLEITEM>>> = {
        let mut r = RwLock::new(HashMap::new());
        r
    };
}

pub fn clear_ongoing_pending() {
    
    if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
        ongoing.clear();
    
    }
    
    if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
        ongoing.clear();
    
    }  

}

pub fn init_sample(symbol:&str) {
    //key: symbol:startts
    let file_path = PAIRMM_CONF.sample_path.to_string()+"/"+symbol+".csv";
    let mut rdr = ReaderBuilder::new()
        .delimiter(b',')  // 设置分隔符，默认为逗号
        .from_path(file_path).unwrap();
    
    if let Some(mut sample_open) = SAMPLEOPEN.try_write_for(Duration::from_secs(1)) {

        for result in rdr.records() {
            let record = result.unwrap();
            // 打印每一行记录
            let k = record.get(1).unwrap().to_string() + ":" + &record.get(5).unwrap().to_string();
            let samlpe_item = SAMPLEITEM{
                lastts:record.get(5).unwrap().to_string().parse::<i64>().unwrap(),
                sid:record.get(6).unwrap().to_string().parse::<i32>().unwrap(),
                sidt:record.get(7).unwrap().to_string().parse::<i32>().unwrap(),
                fkey:record.get(2).unwrap().to_string(),
                side:record.get(8).unwrap().to_string(),
                amount:record.get(4).unwrap().to_string().parse::<f64>().unwrap(),
            };

            
            if sample_open.contains_key(&k) {
                let mut vfs = sample_open.get_mut(&k).unwrap();
                vfs.push(samlpe_item);
            } else {
                //let nh::HashMap<String, Vec<f64>>> = HashMap::new();
                let mut vv = Vec::new();
                vv.push(samlpe_item);
                sample_open.insert(k, vv);                
            }
            

        }
    }    
    
    // let ff = SAMPLEOPEN.read();
    // info!("symbol={} so={:?}", symbol, ff);
}

pub fn cb_init(tinfo:&TradeInfo) {
    info!("cb init {} symbol={}", PAIRMM_CONF.tickpath, tinfo.symbol);
    // init_sample(&tinfo.symbol);

}

pub fn cb_filled(tinfo:&mut TradeInfo, pitem:&PendingItem, filled_amount:f64) {
    info!("cb_filled, fkey={} coid={} amount={}", pitem.from_key, pitem.client_order_id, pitem.amount);
    
    
    if pitem.client_order_id.starts_with("o") {
        
    }  
    else if  pitem.from_key.starts_with("cl_"){
        let ps: Vec<&str> = pitem.from_key.split("_").collect();
        let open_from_key = ps[2];
        let sidc = ps[1].parse::<i32>().unwrap();
        if sidc != pitem.sid {
            panic!("wrong sid");
        }
        
        if pitem.side == "bid" {

            if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
                if ongoing.contains_key(&open_from_key.to_string()) {
                    let mut cvs = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                    let mut deal_amount = 0.;
                    for ci in cvs.citems.iter_mut() {
                        if ci.sidc != pitem.sid {continue;}
                        ci.curr_amount+=filled_amount;
                        deal_amount+=ci.curr_amount;
                        info!("fkey={} coid={} fillamount={} camount={} totalamount={}", open_from_key, pitem.client_order_id, filled_amount, ci.curr_amount, cvs.total_amount);
                    }
                    if deal_amount>= cvs.total_amount {
                        info!("ONGOING_BID drop fkey={}", open_from_key);
                        ongoing.remove(&open_from_key.to_string());
                    }
                }
            }
        }
        else if pitem.side == "ask" {
            if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
                if ongoing.contains_key(&open_from_key.to_string()) {
                    let mut cvs = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                    let mut deal_amount = 0.;
                    for ci in cvs.citems.iter_mut() {
                        if ci.sidc != pitem.sid {continue;}
                        ci.curr_amount+=filled_amount;
                        deal_amount+=ci.curr_amount;
                        info!("fkey={} coid={} fillamount={} camount={} totalamount={}", open_from_key, pitem.client_order_id, filled_amount, ci.curr_amount, cvs.total_amount);
                    }
                    if deal_amount >= cvs.total_amount {
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
        //bid:1:0:0.0004
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
                let mut cvitem = Vec::new();
                let citem = CITEM{sidc:sidt, curr_amount:0.};
                cvitem.push(citem);
                
                
           
                // let cvitem = VCITEM{sidm:sidm,sidt:sidt, sidet:"ask".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, start_close_ts:ts, citems:cvitem};
                let mut start_close_ts = pitem.create_ts/1000;
                if PAIRMM_CONF.is_start_close_open{
                    start_close_ts = pitem.create_ts/1000;
                }else{
                    if ts > 47157326100{
                        start_close_ts = ts/1000;
                    }else if ts > 47157326113780{
                        start_close_ts = ts/1000000;
                    }else{
                        start_close_ts = ts;
                    }                
                }
                // println!("cb_finished===={}==={}=={}",ts,pitem.create_ts/1000,start_close_ts);
                let cvitem = VCITEM{sidm:sidm,sidt:sidt, sidet:"ask".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, start_close_ts:start_close_ts, citems:cvitem};

                info!("cvitem={:?}", cvitem);

                if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
                    ongoing.insert(pitem.from_key.to_string(), cvitem);
                }
   
            }
        }
        else if pitem.side == "ask" {
            let amount_deal = ((pitem.amount_init - pitem.amount) * contract_value_m);
            if amount_deal != 0. {
                let mut cvitem = Vec::new();
                let citem = CITEM{sidc:sidt, curr_amount:0.};
                cvitem.push(citem);
                
                let mut start_close_ts = pitem.create_ts/1000;
                if PAIRMM_CONF.is_start_close_open{
                    start_close_ts = pitem.create_ts/1000;
                }else{
                    if ts > 47157326100{
                        start_close_ts = ts/1000;
                    }else if ts > 47157326113780{
                        start_close_ts = ts/1000000;
                    }else{
                        start_close_ts = ts;
                    }
                }
                // println!("cb_finished===={}==={}=={}",ts,pitem.create_ts/1000,start_close_ts);
                
                // let cvitem = VCITEM{sidm:sidm,sidt:sidt, sidet:"bid".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, start_close_ts:ts, citems:cvitem};
                let cvitem = VCITEM{sidm:sidm,sidt:sidt, sidet:"bid".to_string(), from_key:pitem.from_key.to_string(), total_amount:amount_deal, start_close_ts:start_close_ts, citems:cvitem};
                info!("cvitem={:?}", cvitem);

                if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
                    ongoing.insert(pitem.from_key.to_string(), cvitem);
                }
            }
        } 
        else {
            panic!("wrong side");
        }
    }
}

pub fn get_ongoing_num(sidm:i32, sidt:i32, side:&str)->i32 {
    let mut num = 0;
    if side == "bid" {
        let ongoing = ONGOING_BID.read();
        info!("ongoing bid={:?}", ongoing);
        for (k,ci) in ongoing.iter() {
            if ci.sidt == sidt && ci.sidm == sidm{
                num+=1;
            }
        }
    } else if side == "ask" {
        let ongoing = ONGOING_ASK.read();
        info!("ongoing ask={:?}", ongoing);
        for (k,ci) in ongoing.iter() {
            if ci.sidt == sidt && ci.sidm == sidm{
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
    let dinfo:&DepthInfo = tinfo.depths.get(sid).unwrap();
    // let pos:f64 = *tinfo.pos.get(sid).unwrap();
    if !dinfo.is_finish_snap {
        info!("pitem coid={} rebuild, cancel",pitem.client_order_id);
        return true;
    }
    
//     let e = &ENGIN_CONF.sids[sid];
//     let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
//     let mut contract_value = 1.;
//     if e["etype"] == "swap"  && e["exchange"] == "okx" {
//         contract_value = market.contract_value.unwrap();
//         println!("contract_value={}",contract_value);
//     }
//     let mid:f64 = (dinfo.bid1 + dinfo.ask1) / 2.;
//     let upos:f64 = pos * mid * contract_value;
    
    
    if pitem.client_order_id.starts_with("op_") {
        // println!("======={}",pitem.client_order_id);
        if (t[0]*1000.) as i64 - pitem.create_ts >= (PAIRMM_CONF.max_open_order_keep_s*1000) as i64 {
            info!("pending last too long cid={} o={} c={}", pitem.client_order_id, t[0] as i64, pitem.create_ts);
            return true
        }
        
        // buy_cancel
        // sell_cancel
        if pitem.side == "bid"{
            let buy_cancel = t[*COLS.get(&("buy_cancel".to_string())).unwrap()] as i64;
            if buy_cancel == 1{
                info!("buy_cancel=============");
                return true
            }
            
        }else if pitem.side == "ask"{
            let sell_cancel = t[*COLS.get(&("sell_cancel".to_string())).unwrap()] as i64;
            if sell_cancel == 1{
                info!("sell_cancel=============");
                return true
            }
        }
        
        // if PAIRMM_CONF.is_opne_price_gap_ratio{
        //     let bid1:f64 = dinfo.bid1;
        //     let ask1:f64 = dinfo.ask1;
        //     let mut price_gap_ratio = 0.0;
        //     if pitem.side == "bid"{
        //         price_gap_ratio = (bid1 - pitem.price)/bid1;
        //     }else if pitem.side == "ask"{
        //         price_gap_ratio = (pitem.price - ask1)/ask1;
        //     }
        //     // println!("is_cancel======={}",price_gap_ratio);
        //     if price_gap_ratio > PAIRMM_CONF.opne_price_gap_ratio{
        //         println!("is_cancel======={}",price_gap_ratio);
        //         return true
        //     }
        // }
    }


    // let e = &ENGIN_CONF.sids[sid];
    // let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    
    if pitem.from_key.starts_with("cl_") {
        // println!("cl======={}=={}",pitem.from_key,pitem.client_order_id);
        if (t[0]*1000.) as i64 - pitem.create_ts >= (PAIRMM_CONF.max_close_order_keep_s*1000) as i64 {
            info!("pending last too long cid={} o={} c={}", pitem.client_order_id, t[0] as i64, pitem.create_ts);
            return true
        }
        
        
        let ps: Vec<&str> = pitem.from_key.split("_").collect();
        let open_from_key = ps[2];
        let sidc = ps[1].parse::<i32>().unwrap();
        if sidc != pitem.sid {
            panic!("wrong sid");
        }
        
        if pitem.side == "bid" {   
            if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
                if !ongoing.contains_key(&open_from_key.to_string()) {
                    info!("pkey={} not in the ongoing, cancel, oid={}", open_from_key, pitem.client_order_id);
                    return true;
                }

                
                let mut cvs = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                let mut total_deal = 0.;
                for ci in cvs.citems.iter_mut() {
                    total_deal+=ci.curr_amount;
                }
                if total_deal >= cvs.total_amount {
                    info!("pkey={} already finished[{} {}], cancel, oid={}", open_from_key, total_deal, cvs.total_amount, pitem.client_order_id);
                    return true;
                }
            }      
        }
        else if pitem.side == "ask" {   
            if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
                if !ongoing.contains_key(&open_from_key.to_string()) {
                    info!("pkey={} not in the ongoing, cancel, oid={}", open_from_key, pitem.client_order_id);
                    return true;
                }
                
                let mut cvs = ongoing.get_mut(&open_from_key.to_string()).unwrap();
                let mut total_deal = 0.;
                for ci in cvs.citems.iter_mut() {
                    total_deal+=ci.curr_amount;
                }
                if total_deal >= cvs.total_amount {
                    info!("pkey={} already finished[{} {}], cancel, oid={}", open_from_key, total_deal, cvs.total_amount, pitem.client_order_id);
                    return true;
                }
            }      
        }
        else {panic!("side");}
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
                        let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price, client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
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

pub fn get_target_prem_by_ranges(side:&str,ranges:Vec<f64>, sf:f64, is_open:bool) ->Vec<f64>{
    let mut vs:Vec<f64> = Vec::new();
    
    for range in ranges {
        
        vs.push(range);
        
    }
    return vs;
}

pub fn get_range_of_pstat(ts:i64, side:&str, sid:i32)->Vec<f64> {
    //pstat
    //let k = record.get(1).unwrap().to_string() + ":" + &record.get(2).unwrap().to_string() + ":" + &record.get(3).unwrap().to_string();
    let mut vs:Vec<f64> = vec![0.0,0.0002, 0.0006, 0.001];
    return vs;
}

pub fn get_oppo_close_ability(ts:i64, sid:i32) -> bool{
    return true;
}

pub fn get_rbs_from_df_maker_close(t:&Vec<f64>, df:f64,  side:&str) ->f64 {
    let mut rs = -1.;
    
    if df > PAIRMM_CONF.close_df {
        return rs;
    }
    
    let target_thr_maker_close = 0.0004;
    
    if side == "buy" {        
        
        let rbs:Vec<f64> = vec![0., 0.0002, 0.0004,0.0006,0.0008,0.001];
        for rb in rbs {
            
            let prem = rb + (-df);
            if prem > target_thr_maker_close{
                rs = rb;
                break;
            }
        }

        return rs;
    } else if side == "sell" {
     
        let rbs:Vec<f64> = vec![0., 0.0002,0.0004,0.0006,0.0008,0.001];

        for rb in rbs {
            let prem = rb + (-df);
            if prem > target_thr_maker_close{
                rs = rb;
                break
            }
        }
        info!("f={} rs={}", df, rs);
        return rs;
    } else {
        panic!("no side={}", side);
    }

    return rs   
}

pub fn get_close_decision(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision> {
    let mut tds:Vec<MakeDecision> = Vec::new();
    
    for sid in ENGIN_CONF.vsids.iter() {

        
        let last_y:f64 = 60000.;
        let last_rb_start:f64 = 0.003;
        let last_rb_end:f64 = -0.003;
        
        let ongoing = ONGOING_BID.read();
        for (k,ci) in ongoing.iter() {
            if ci.sidt != *sid {
                continue;
            }
            //计算总共的已成交
            let mut total_deal = 0.;
            for citem in ci.citems.iter() {
                total_deal+=citem.curr_amount;
            }
            if total_deal >= ci.total_amount {
                continue;
            }
            
            for citem in ci.citems.iter() {
                let e_m = &ENGIN_CONF.sids[&citem.sidc];
                let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
                let dinfo_m:&DepthInfo = tinfo.depths.get(&citem.sidc).unwrap();

                let mut contract_value = 1.;
                if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
                    contract_value = market_m.contract_value.unwrap();
                }
                let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;
                if !dinfo_m.is_finish_snap  || mid_m == 0. {
                    continue
                }
                
                let curr_last = (t[0] as i32 - (ci.start_close_ts) as i32) as f64;
                                
                if (curr_last < PAIRMM_CONF.close_ts){
                    continue
                }
                
                
                
//                 let range_delta = linear_interpolation(0., last_rb_start, last_y , last_rb_end, curr_last);
                
                let amount = (ci.total_amount - total_deal) / contract_value;
//                 let key = "f_".to_string() + &citem.sidc.to_string() + "_" + &ci.sidm.to_string();
//                 let f = t[*COLS.get(&key).unwrap()] as f64;
                
//                 let rb_close = get_rbs_from_df_maker_close(t, f, "buy");
//                 info!("fkey={} key={} f={} rb_close={}", ci.from_key, key, f, rb_close);
//                 if rb_close == -1. {
//                     continue;
//                 }
//                 let mut rb = rb_close + range_delta;
//                 rb = rb.max(0.);
//                 info!("fkey={} rb_close={} range_delta={} rb={}", ci.from_key, rb_close, range_delta, rb);
                
                let rb = PAIRMM_CONF.close_rb;
                let mut pkey = "cl_".to_string() + &citem.sidc.to_string() + "_" + &ci.from_key;
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "cbid_m_".to_string()+&ci.sidm.to_string()+"_"+&ci.sidt.to_string()+"_"+&citem.sidc.to_string()+"_"+&rb.to_string()+"_"+ &curr_cid.to_string();
                let nbids = get_pending_num_from_key_bid(citem.sidc, &pkey);
                info!("pkey={} nbids={}", pkey, nbids);
                
                if nbids > 0 {
                    continue;
                }
                
                
                let bid_price:f64 = dinfo_m.bid1 * (1. - rb);
                info!("bid sidc={} [{} {}] bid_price={} rb={}", citem.sidc, dinfo_m.bid1, dinfo_m.ask1, bid_price, rb);
                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                    client_order_id:client_order_id, 
                    max_order_keep_s:PAIRMM_CONF.max_close_order_keep_s as i32, 
                    side:"buy".to_string(), 
                    sid:citem.sidc, 
                    ttype:"maker".to_string(), 
                    price:bid_price, 
                    amount:amount, 
                    from_key:pkey.to_string(),
                    dup_key:"".to_string(),
                    target_sid:-1
                });
            }
        }
        
        //ask
        let ongoing = ONGOING_ASK.read();
        //info!()
        for (k,ci) in ongoing.iter() {
            if ci.sidt != *sid {
                continue;
            }
            //计算总共的已成交
            let mut total_deal = 0.;
            for citem in ci.citems.iter() {
                total_deal+=citem.curr_amount;
            }
            //info!("fkey={} total_deal={} total_amount={}", k, total_deal, ci.total_amount);
            if total_deal >= ci.total_amount {
                
                continue;
            }
            
            for citem in ci.citems.iter() {
                let e_m = &ENGIN_CONF.sids[&citem.sidc];
                let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
                let dinfo_m:&DepthInfo = tinfo.depths.get(&citem.sidc).unwrap();

                let mut contract_value = 1.;
                if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
                    contract_value = market_m.contract_value.unwrap();
                }
                let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;

                if !dinfo_m.is_finish_snap  || mid_m == 0. {
                    continue
                }
                // let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;

                let curr_last = (t[0] as i32 - (ci.start_close_ts) as i32) as f64;
                if (curr_last < PAIRMM_CONF.close_ts){
                    continue
                }
                
                // let range_delta = linear_interpolation(0., last_rb_start, last_y , last_rb_end, curr_last);
                
                let amount = (ci.total_amount - total_deal) / contract_value;
//                 let key = "f_".to_string() + &citem.sidc.to_string() + "_" + &ci.sidm.to_string();
//                 // println!("key={}", key);
//                 let f = t[*COLS.get(&key).unwrap()] as f64;
                
//                 let rb_close = get_rbs_from_df_maker_close(t, -f, "sell");
//                 info!("fkey={} key={} f={} rb_close={}", ci.from_key, key, f, rb_close);

//                 if rb_close == -1. {
//                     continue;
//                 }
//                 let mut rb = rb_close + range_delta;
//                 rb = rb.max(0.);
//                 info!("fkey={} rb_close={} range_delta={} rb={}", ci.from_key, rb_close, range_delta, rb);

                let rb = PAIRMM_CONF.close_rb;
                let mut pkey = "cl_".to_string() + &citem.sidc.to_string() + "_" + &ci.from_key;
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "cask_m_".to_string()+&ci.sidm.to_string()+"_"+&ci.sidt.to_string()+"_"+&citem.sidc.to_string()+"_"+&rb.to_string()+"_"+ &curr_cid.to_string();
                let nasks = get_pending_num_from_key_ask(citem.sidc, &pkey);
                info!("close fkey={}, pkey={} sidc={} rb={} client_order_id={} nasks={}", ci.from_key, pkey, citem.sidc, rb, client_order_id, nasks);

                if nasks > 0 {
                    continue;
                }
                
                let ask_price:f64 = dinfo_m.ask1 * (1. + rb);
                info!("ask sidc={} [{} {}] ask_price={} rb={} ", citem.sidc, dinfo_m.bid1, dinfo_m.ask1, ask_price, rb);
                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                    client_order_id:client_order_id, 
                    max_order_keep_s:PAIRMM_CONF.max_close_order_keep_s as i32, 
                    side:"sell".to_string(), 
                    sid:citem.sidc, 
                    ttype:"maker".to_string(), 
                    price:ask_price, 
                    amount:amount, 
                    from_key:pkey.to_string(),
                    dup_key:"".to_string(),
                    target_sid:-1
                });
            }
        } 
    } 
    
    info!("close make decision={:?}", tds);
    return tds;
}

pub fn sample_onging(t:&Vec<f64>, tinfo:&mut TradeInfo) {
//     let symbol:&str = &tinfo.symbol;
//     let key = symbol.to_string() + ":" + &t[0].to_string();
//     let samples = SAMPLEOPEN.read();
//     //info!("key={}", key,);
//     if samples.contains_key(&key) {
//         let vsample = samples.get(&key).unwrap();
//         info!("key={} hit v={:?}", key, vsample);
//         for sitem in vsample.iter() {
//             let mut oside = "";
//             if sitem.side == "buy" {
//                 oside = "ask"; 
//                 let mut cvitem = Vec::new();
//                 for sidc in ENGIN_CONF.vsids.iter() {
//                     if *sidc == sitem.sid {
//                         continue;
//                     }
//                     let citem = CITEM{sidc:*sidc, curr_amount:0.};
//                     cvitem.push(citem);
//                 }

//                 let cvitem = VCITEM{sidm:sitem.sid,sidt:sitem.sidt, sidet:oside.to_string(), from_key:sitem.fkey.to_string(), total_amount:sitem.amount, start_close_ts:sitem.lastts, citems:cvitem};
//                 if let Some(mut ongoing) = ONGOING_ASK.try_write_for(Duration::from_secs(1)) {
//                     ongoing.insert(sitem.fkey.to_string(), cvitem);
//                 }
//             }
//             else if sitem.side == "sell" {
//                 oside = "bid"; 
//                 let mut cvitem = Vec::new();
//                 for sidc in ENGIN_CONF.vsids.iter() {
//                     if *sidc == sitem.sid {
//                         continue;
//                     }
//                     let citem = CITEM{sidc:*sidc, curr_amount:0.};
//                     cvitem.push(citem);
//                 }
//                 let cvitem = VCITEM{sidm:sitem.sid,sidt:sitem.sidt, sidet:oside.to_string(), from_key:sitem.fkey.to_string(), total_amount:sitem.amount, start_close_ts:sitem.lastts, citems:cvitem};
//                 if let Some(mut ongoing) = ONGOING_BID.try_write_for(Duration::from_secs(1)) {
//                     ongoing.insert(sitem.fkey.to_string(), cvitem);
//                 }
//             }
//             else {
//                 panic!("side");
//             }

//         }
        
//     }
    
}

fn count_decimal_places(num: f64) -> usize {
    let num_str = num.to_string();
    if let Some(decimal_index) = num_str.find('.') {
        num_str.len() - decimal_index - 1
    } else {
        0
    }
}

pub fn get_open_decision(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision> {
    let mut tds:Vec<MakeDecision> = Vec::new();
    let max_pos_u = PAIRMM_CONF.amountu * 500.;
    let max_ongoing = 5;
    
    let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
    // println!("============={}",ts);
    for sid in ENGIN_CONF.vsids.iter() { 
        if sid != &PAIRMM_CONF.opne_sid {
            continue 
        } 
        
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
        if !dinfo_m.is_finish_snap || mid_m == 0. {
            continue
        }
        
        let (nbids,nasks) = get_pending_num_from_sid(*sid);
                     
        let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
        let signal1 = t[*COLS.get(&("signal1".to_string())).unwrap()] as i64;
        let signal2 = t[*COLS.get(&("signal2".to_string())).unwrap()] as i64;
        let signal3 = t[*COLS.get(&("signal3".to_string())).unwrap()] as i64;
        //bid
        for sidt in ENGIN_CONF.vsids.iter() {
            if sid == sidt {
                continue;
            }
            
            // let ongoing_num = get_ongoing_num(*sid, *sidt, "bid") as f64;
            
            if upos_m + (nbids as f64) * PAIRMM_CONF.amountu > PAIRMM_CONF.max_pos_u  {
                info!(" 不挂单 bib pos_m={} mid_m={} upos_m={} ongoing_num={}", pos_m,mid_m,upos_m,nbids);
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
                // println!("22===dinfo_m.is_finish_snap============={}",dinfo_m.is_finish_snap);
                continue
            }
            
            let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
            info!("sidt={} amt={} cv={}", sidt, amount_hand_token, contract_value_t);
            let upos_t:f64 = pos_t * contract_value_t * mid_t;

//                 let key = "f_".to_string() + &sid.to_string() + "_" + &sidt.to_string();
//                 let f = t[*COLS.get(&key).unwrap()] as f64;

//                 let ranges_bids = get_range_of_pstat(t[0] as i64, "bid", *sid);
//                 let ranges_asks = get_range_of_pstat(t[0] as i64, "ask", *sid);
            let mut is_open = true;

            let ranges_bid_prem = &PAIRMM_CONF.open_ranges;
            // get_target_prem_by_ranges("buy", ranges_bids, f, is_open);

            // # signal1 signal2 signal3 
            
            if signal1 == 1 && signal2 == 1 && signal3 ==1{
                for range in ranges_bid_prem {
                    let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
                    let fkey = "bid:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string()+":"+&signal1.to_string() +":"+&signal2.to_string() +":"+&signal2.to_string()+":"+&ts.to_string();
                    let dupkey = "bid:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string()+":"+&signal1.to_string()+":"+&signal2.to_string()+":"+&signal3.to_string();
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "op_obid".to_string()+":"+&sid.to_string()+":"+&sidt.to_string()+"_"+&range.to_string()+"_"+&signal1.to_string()+"_"+&signal2.to_string()+"_"+&signal3.to_string()+"_"+ &curr_cid.to_string();
                    let bid_price:f64 = dinfo_m.bid1 * (1. - range);     
                    tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                        client_order_id:client_order_id, 
                        max_order_keep_s:PAIRMM_CONF.max_open_order_keep_s as i32, 
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
            
            // let ongoing_num = get_ongoing_num(*sid, *sidt, "ask") as f64;
            
            if upos_m - ((nasks as f64) * PAIRMM_CONF.amountu ) < PAIRMM_CONF.max_pos_u * -1.0{
                info!(" 不挂单 ask pos_m={} mid_m={} upos_m={} ongoing_num={}", pos_m,mid_m,upos_m,nasks);
                 continue;
            }

            let e_t = &ENGIN_CONF.sids[sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&sidt).unwrap();
            let mut contract_value_t = 1.;
            if e_t["etype"] == "swap"  && e_t["exchange"] == "okx" {
                // println!("33===dinfo_m.is_finish_snap============={}",dinfo_m.is_finish_snap);
                contract_value_t = market_t.contract_value.unwrap();
            }

            let mid_t:f64 = (dinfo_t.bid1 + dinfo_t.ask1) / 2.;
            if !dinfo_t.is_finish_snap  || mid_t == 0. {
                // println!("44===dinfo_m.is_finish_snap============={}",dinfo_m.is_finish_snap);
                continue
            }
            let amount_hand_token = PAIRMM_CONF.amountu / contract_value_m / mid_m;
            info!("sidt={} amt={} cv={}", sidt, amount_hand_token, contract_value_t);
            let upos_t:f64 = pos_t * contract_value_t * mid_t;
            let mut is_open = true;
            let ranges_ask_prem = &PAIRMM_CONF.open_ranges;
            
           
            if signal1 == -1 && signal2 == 1 && signal3 == 1{
                for range in ranges_ask_prem {                        
                    let fkey = "ask:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string()+":"+&signal1.to_string()+":"+&signal2.to_string()+":"+&signal3.to_string()+":"+&ts.to_string();
                    let dupkey = "ask:".to_string()+&sid.to_string()+":"+&sidt.to_string()+":"+&range.to_string()+":"+&signal1.to_string()+":"+&signal2.to_string()+":"+&signal3.to_string();
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "op_oask".to_string()+":"+&sid.to_string()+":"+&sidt.to_string()+"_"+&range.to_string()+"_"+&signal1.to_string()+"_"+&signal2.to_string()+"_"+&signal3.to_string()+"_"+ &curr_cid.to_string();
                    let ask_price:f64 = dinfo_m.ask1 * (1. + range);                
                    tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                        client_order_id:client_order_id, 
                        max_order_keep_s:PAIRMM_CONF.max_open_order_keep_s as i32, 
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
    
    info!("open make decision={:?}", tds);
    return tds;
}

pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    
    let pairpos_item_u = 50. * PAIRMM_CONF.amountu;
    let max_pos_u = PAIRMM_CONF.amountu * 500.;
    let max_pending_pair = 1;
    let max_ongoing = 5;
    let ts = t[0];
    let mut tds:Vec<MakeDecision> = Vec::new();
    
    //sample_onging(t, tinfo);
    let tds_close = get_close_decision(t, tinfo);
    let tds_open = get_open_decision(t, tinfo);
    tds.extend(tds_open);
    tds.extend(tds_close);

    return tds;
} 

