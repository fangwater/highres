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
use parking_lot::RwLock;


use rust_decimal::Decimal;
use std::str::FromStr;
use rust_decimal::prelude::*;

#[derive(Debug)]
pub struct CITEM {
    pub tsid:i32,
    pub tside:String,
    pub total_amount:f64,
    pub curr_amount:f64,
    pub start_close_ts:i64,
}

lazy_static! {
    static ref COLS: HashMap<String, usize> = {
        let mut map = HashMap::new();
        map.insert("ts".to_string(), 0);
        map.insert("f_0".to_string(), 1);
        map.insert("fm_0".to_string(), 2);
        map.insert("s_0".to_string(), 3);
        map.insert("sb_0".to_string(), 4);
        map.insert("ss_0".to_string(), 5);
        
        map.insert("f_1".to_string(), 6);
        map.insert("fm_1".to_string(), 7);
        map.insert("s_1".to_string(), 8);
        map.insert("sb_1".to_string(), 9);
        map.insert("ss_1".to_string(), 10);
        

        map
    };
    
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);
    
    static ref TOCLOSE: RwLock<HashMap<String,CITEM>> = {
        let mut map = HashMap::new();
        
        RwLock::new(map)
    };

    
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

fn count_decimal_places(num: f64) -> usize {
    let num_str = num.to_string();
    if let Some(decimal_index) = num_str.find('.') {
        num_str.len() - decimal_index - 1
    } else {
        0
    }
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
        if df > 0. {
            return rs;
        }
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


pub fn get_rbs_from_df_maker_open(t:&Vec<f64>, df:f64,  side:&str) ->Vec<f64> {
    let mut rs:Vec<f64> = Vec::new();
    if df > 0. {
        return rs;
    }
    
    let target_thr_maker_open = 0.0002;
    
    
    if side == "buy" {        
        
        let rbs:Vec<f64> = vec![0.0002,0.0004,0.0006,0.0008,0.0015];
        for rb in rbs {
            
            let prem = rb + (-df);
            if prem > target_thr_maker_open{
                rs.push(rb);
            }
        }

        return rs;
    } else if side == "sell" {
     
        let rbs:Vec<f64> = vec![0.0002,0.0004,0.0006,0.0008,0.0015];
        if df > 0. {
            return rs;
        }
        for rb in rbs {
            let prem = rb + (-df);
            if prem > target_thr_maker_open{
                rs.push(rb);
            }
        }

        return rs;
    } else {
        panic!("no side={}", side);
    }
    
    return rs
    
    
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

pub fn cb_finished(ts:i64, tinfo:&mut TradeInfo, pitem:&PendingItem, is_cancel:bool) {
    let ps: Vec<&str> = pitem.from_key.split(":").collect();

    info!("finished coid={} is_cancel={} ts={}", pitem.client_order_id, is_cancel, ts);
    if pitem.from_key.starts_with("o") {

        println!("pitem.from_key={}", pitem.from_key);
        let sid:i32 = ps[3].parse::<i32>().unwrap();

        let mut tsid = -1;
        if sid == 0 {
            tsid = 1;
        }
        else if sid == 1 {
            tsid = 0;
        }
        else {
            panic!("tsid sid={}", sid);
        }

        info!("pitem.amount_init={} pitem.amount={}", pitem.amount_init, pitem.amount);

//         if pitem.amount < pitem.amount_init {
            
//             return
//         }

        //add
        //info!("add to wait-to-close fromkey={}", pitem.from_key);

        let mut tside = "";
        if pitem.side == "bid" {
            tside = "sell";
        }
        else if pitem.side == "ask" {
            tside = "buy"
        }
        else {
            panic!("tside, side={}", pitem.side)
        }

        if let Some(mut toclose) = TOCLOSE.try_write_for(Duration::from_secs(1)) {
            let citem = CITEM{
                tsid:tsid,
                tside:tside.to_string(),
                total_amount:pitem.amount_init,
                curr_amount:0.,
                start_close_ts:ts/1000,
            };
            toclose.insert(pitem.from_key.to_string(), citem);
        }
    }
}


pub fn cb_filled(tinfo:&mut TradeInfo, pitem:&PendingItem, filled_amount:f64) {
    let ps: Vec<&str> = pitem.from_key.split(":").collect();
    

    if  pitem.from_key.starts_with("ss") {
        //close
        let ps: Vec<&str> = pitem.from_key.split("ss").collect();
        let fkey = ps[1].to_string();
        info!("close deal, fkey={} filled_amount={}", fkey, filled_amount);
        if let Some(mut toclose) = TOCLOSE.try_write_for(Duration::from_secs(1)) {
            if let Some(mut s) = toclose.get_mut(&fkey) {
                s.curr_amount += filled_amount;
                info!("fkey={} camount={} tamount={}", fkey, s.curr_amount, s.total_amount);
                if s.curr_amount >= s.total_amount {
                    // 如果当前数量达到或超过总数量，则移除
                    toclose.remove(&fkey);
                }
            } else {
                // 如果没有找到 fkey 对应的值，你可以在这里处理，或者什么也不做继续执行
                info!("fkey={} not found, skipping...", fkey);
            }

            
            // let mut s = toclose.get_mut(&fkey).unwrap();
            // s.curr_amount+=filled_amount;
            // info!("fkey={} camount={} tamount={}", fkey, s.curr_amount, s.total_amount);
            // if s.curr_amount >= s.total_amount {
            //     //drop
            //     toclose.remove(&fkey);
            // }
        }
    }

    
}



pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    let mut tds:Vec<MakeDecision> = Vec::new();
    
    //close
    for sid in ENGIN_CONF.vsids.iter() {
        let e = &ENGIN_CONF.sids[&sid];
        let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

        
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
        
        let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
        let df = t[*COLS.get(&("fm_".to_string()+&sid.to_string())).unwrap()];
        let fstd = t[*COLS.get(&("s_".to_string()+&sid.to_string())).unwrap()];
            
        let amount_hand_token = ARBMM2_CONF.amountu / contract_value / mid;
        let last_y:f64 = 60.;
        let last_rb_start:f64 = 0.003;
        let last_rb_end:f64 = -0.003;
        

        
        for (fkey, citem) in TOCLOSE.read().iter() {
            let curr_last = (ts - citem.start_close_ts) as f64;
            info!("ts={} start_close={} last={}", ts, citem.start_close_ts, curr_last);
            let range_delta = linear_interpolation(0., last_rb_start, last_y , last_rb_end, curr_last);

            
            if citem.tsid != *sid {
                continue;
            }
            
            let amount = citem.total_amount - citem.curr_amount;
            
            if citem.tside == "buy" {

                let rb_close = get_rbs_from_df_maker_close(t, df, "buy");
                let mut rb = rb_close + range_delta;
                
                rb = rb.max(0.);

                
                if rb_close == -1. {
                    continue;
                }
                let mut rb_bid = rb;
                
                let pkey = "ss".to_string() + &fkey;
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "cbid_m_".to_string()+&sid.to_string()+"_"+&rb_bid.to_string()+"_"+ &curr_cid.to_string();
                
                let nbids = get_pending_num_from_key_bid(*sid, &pkey);
                info!("fkey={}, nbids={}", pkey, nbids);
                if nbids > 0 {
                    continue;
                }
                
                let bid_price:f64 = dinfo.bid1 * (1. - rb_bid);
                info!("bid curr_last={} rb_close={} range_delta={} rb_bid={} bid1={} bid_price={}", curr_last, rb_close, range_delta, rb_bid, dinfo.bid1, bid_price);
                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                    client_order_id:client_order_id, 
                    max_order_keep_s:ARBMM2_CONF.max_order_keep_s, 
                    side:"buy".to_string(), 
                    sid:*sid, 
                    ttype:"maker".to_string(), 
                    price:bid_price, 
                    amount:amount, 
                    from_key:pkey.to_string(),target_sid:-1
                });
            }
            else if citem.tside == "sell" {
                let rb_close = get_rbs_from_df_maker_close(t, -df, "sell");
                let mut rb = rb_close + range_delta;
                rb = rb.max(0.);

                
                if rb_close == -1. {
                    continue;
                }
                
                let mut rb_ask = rb;
                let pkey = "ss".to_string() + &fkey;
                let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                let client_order_id = "cask_m_".to_string()+&sid.to_string()+"_"+&rb_ask.to_string()+"_"+ &curr_cid.to_string();
                
                let nasks = get_pending_num_from_key_ask(*sid, &pkey);
                info!("fkey={}, nasks={}", pkey, nasks);
                if nasks > 0 {
                    continue;
                }
                
                let ask_price:f64 = dinfo.ask1 * (1. + rb_ask);
                info!("ask curr_last={} rb_close={} range_delta={} rb_ask={} ask1={} ask_price={}", curr_last, rb_close, range_delta, rb_ask, dinfo.ask1, ask_price);

                tds.push(MakeDecision{create_ts:t[*COLS.get("ts").unwrap()] as i64,
                    client_order_id:client_order_id, 
                    max_order_keep_s:ARBMM2_CONF.max_order_keep_s, 
                    side:"sell".to_string(), 
                    sid:*sid, 
                    ttype:"maker".to_string(), 
                    price:ask_price, 
                    amount:amount, 
                    from_key:pkey.to_string(),target_sid:-1
                });
            }
            

        }
        
    }
    
    
    
    //pair sampling
    //open 采样规则：每 60s一判断, 大于3bp
    for sid in ENGIN_CONF.vsids.iter() {
        let e = &ENGIN_CONF.sids[&sid];
        let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

        
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
        
        let ts = t[*COLS.get(&("ts".to_string())).unwrap()] as i64;
        let df = t[*COLS.get(&("fm_".to_string()+&sid.to_string())).unwrap()];
        let fstd = t[*COLS.get(&("s_".to_string()+&sid.to_string())).unwrap()];
            
        if ts % 60 != 0 {
            continue;
        }
        
        let amount_hand_token = ARBMM2_CONF.amountu / contract_value / mid;
        
        
        let rb_bid_dfs = get_rbs_from_df_maker_open(t, df, "buy");
        let rb_ask_dfs = get_rbs_from_df_maker_open(t, -df, "sell");
        
  
        
        for rb_bid_df in &rb_bid_dfs {
            let mut rb_bid = rb_bid_df;
            let pkey = "obid:".to_string()+":"+&rb_bid_df.to_string()+":"+&sid.to_string()+":"+&ts.to_string();    

            let amount_token = amount_hand_token;
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
        
        

        for rb_ask_df in &rb_ask_dfs {
            let mut rb_ask = rb_ask_df;
        
            let pkey = "oask:".to_string()+":"+&rb_ask_df.to_string()+":"+&sid.to_string()+":"+&ts.to_string();    

            let amount_token = amount_hand_token ;
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

pub fn is_cancel_close(t:&Vec<f64>, tinfo:&mut TradeInfo, pitem:&PendingItem, sid:&i32) -> bool{
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
        info!("close pending last too long cid={} o={} c={}", pitem.client_order_id, t[0] as i64, pitem.create_ts);
        return true
    }

    
    return false
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
        info!("pending last too long cid={} o={} c={}", pitem.client_order_id, t[0] as i64, pitem.create_ts);
        return true
    }

    let gopenu = tinfo.open * mid;
    let gopen_ctl_max = ARBMM2_CONF.amountu * 1000.;
    let full_pos = 200. * ARBMM2_CONF.amountu;
    let rb_bid_openrisk = linear_interpolation(-gopen_ctl_max, -0.005, gopen_ctl_max, 0.005, gopenu);
    let rb_ask_openrisk = linear_interpolation(-gopen_ctl_max, 0.005, gopen_ctl_max, -0.005, gopenu);

    let rb_bid_posctl = linear_interpolation(-full_pos as f64, -0.0005, full_pos as f64, 0.0005, upos);
    let rb_ask_posctl = linear_interpolation(-full_pos as f64, 0.0005, full_pos as f64, -0.0005, upos);
    
        

    let e = &ENGIN_CONF.sids[sid];
    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    
    let tick_size = market.precision.tick_size;
    let tick_size_keep = count_decimal_places(tick_size) as u32;
    let rd = RoundingStrategy::ToZero;
    let ru = RoundingStrategy::AwayFromZero;

    let pairkey = pitem.from_key.to_string().replace(":", "_");
    let df = t[*COLS.get(&("fm_".to_string()+&sid.to_string())).unwrap()];
    let fstd = t[*COLS.get(&("s_".to_string()+&sid.to_string())).unwrap()];
    
    //get df
    if pitem.side == "bid" {
        //计算如果还在溢价区间内， 则不动, 
        //TODO 这里还需要考虑last时间 
        
        let curr_rb = (dinfo.bid1 - pitem.price) / dinfo.bid1;
        let prem = curr_rb + (-df);
        if prem < 0.0001 || -df < 0.{
            return true;
        }

    } else if pitem.side == "ask" {
        let curr_rb = (pitem.price - dinfo.ask1) / dinfo.ask1;
        let prem = curr_rb + df;
        if prem < 0.0001 || df < 0.{
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
                    if pitem.from_key.starts_with("o") {
                        if is_cancel(t, tinfo, pitem, &sid) {
                            let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                            cds.push(v);
                        }
                    } else if pitem.from_key.starts_with("ss") {
                        if is_cancel_close(t, tinfo, pitem, &sid) {
                            let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                            cds.push(v);
                        }
                    }
                    else {
                        panic!("fromkey")
                    }
                    
                }

            }
        }
        
        if let Some(porders) = S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1)) {
            for (_price, vpitem) in porders.phash.iter() {
                for pitem in vpitem.iter() {
                    if pitem.from_key.starts_with("o") {
                        if is_cancel(t, tinfo, pitem, &sid) {
                            let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                            cds.push(v);
                        }
                    }
                    else if pitem.from_key.starts_with("ss") {
                        if is_cancel_close(t, tinfo, pitem, &sid) {
                            let v:CancelDecision = CancelDecision{sid:*sid, side:pitem.side.to_string(), price:pitem.price ,client_order_id:pitem.client_order_id.to_string(), delayup_ms:msts + ENGIN_CONF.cancel_delay_ms};
                            cds.push(v);
                        }
                    }
                    else {
                        panic!("fromkey")
                    }
                }

            }
        }
    }
    
    return cds;

}

