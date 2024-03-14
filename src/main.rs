mod gconf;
mod tprocess;
mod lprocess;
mod trade;
mod stgs;
mod stg;
mod spending;
mod symbolinfo;
mod record;
mod ordertake;
mod outsplit;

extern crate ndarray;
extern crate ndarray_npy;
use ndarray::Data;
use log4rs;
use log::{info, debug};
use chrono::{NaiveDate, Duration};
use std::{fs::File};
use ndarray::prelude::*;
use ndarray::Array2;
use ndarray_npy::ReadNpyExt;
use ordered_float::OrderedFloat;
use std::fs;



use crate::gconf::{ENGIN_CONF};
use crate::trade::{MakeDecision, CancelDecision, TradeInfo, DepthInfo};
use crate::spending::{add_pending, clear_pending, drop_pending,S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS, PendingItem};
use crate::ordertake::{add_taking};
use crate::record::{write_to_csv, RecordDumpItem};
use crate::symbolinfo::get_market;
use crate::outsplit::{split_csv_file};



fn arraybase_to_vec<T, S>(array: ArrayBase<S, Ix1>) -> Vec<T>
where
    T: Clone,
    S: Data<Elem = T>,
{
    array.iter().cloned().collect()
}

fn record_pendings(tsms:i64, tinfo:&mut TradeInfo) {

    
    for sid in &ENGIN_CONF.vsids {
        if let Some(mut porders) = S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(std::time::Duration::from_secs(1)) {
            let mut vdel:Vec<OrderedFloat<f64>> = Vec::new();
            for (price, vpitem) in porders.phash.iter() {
                for pitem in vpitem {
                    let e = &ENGIN_CONF.sids[sid];
                    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

                    let mut contract_value = 1.;
                    if e["etype"] == "swap"  && e["exchange"] == "okx" {
                        contract_value = market.contract_value.unwrap();
                        println!("contract_value={}",contract_value);
                    }
                    
                    

                    if ENGIN_CONF.is_spending_tick_dump {
                        let dinfo:&mut DepthInfo = tinfo.depths.get_mut(sid).unwrap();
                        let update_ts_ms = tsms;
                        let rd:RecordDumpItem = RecordDumpItem{create_ts:pitem.create_ts,
                            update_ts:update_ts_ms,
                            client_order_id:pitem.client_order_id.to_string(),
                            symbol:tinfo.symbol.to_string(), 
                            ttype:"maker".to_string(), 
                            sid:*sid, 
                            side:pitem.side.to_string(), 
                            price:pitem.price, 
                            amount_init:pitem.amount_init*contract_value, 
                            amount_update:0., 
                            tlen:pitem.tlen,
                            inpos:pitem.inpos,
                            status:"tickupdate".to_string(), 
                            from_key:pitem.from_key.to_string(),
                            bid1:dinfo.bid1,
                            ask1:dinfo.ask1
                            };
                        write_to_csv(&rd);
                    }
                }
            }
        }

        if let Some(mut porders) = S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(std::time::Duration::from_secs(1)) {
            let mut vdel:Vec<OrderedFloat<f64>> = Vec::new();
            for (price, vpitem) in porders.phash.iter() {
                for pitem in vpitem {
                    let e = &ENGIN_CONF.sids[sid];
                    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();

                    let mut contract_value = 1.;
                    if e["etype"] == "swap"  && e["exchange"] == "okx" {
                        contract_value = market.contract_value.unwrap();
                        println!("contract_value={}",contract_value);
                    }

                    if ENGIN_CONF.is_spending_tick_dump {
                        
                        let dinfo:&mut DepthInfo = tinfo.depths.get_mut(sid).unwrap();

                        let update_ts_ms = tsms;
                        let rd:RecordDumpItem = RecordDumpItem{create_ts:pitem.create_ts,
                            update_ts:update_ts_ms,
                            client_order_id:pitem.client_order_id.to_string(),
                            symbol:tinfo.symbol.to_string(), 
                            ttype:"maker".to_string(), 
                            sid:*sid, 
                            side:pitem.side.to_string(), 
                            price:pitem.price, 
                            amount_init:pitem.amount_init*contract_value, 
                            amount_update:0., 
                            tlen:pitem.tlen,
                            inpos:pitem.inpos,
                            status:"tickupdate".to_string(), 
                            from_key:pitem.from_key.to_string(),
                            bid1:dinfo.bid1,
                            ask1:dinfo.ask1
                        };
                        write_to_csv(&rd);
                    }
                }
            }
        }
    }
}

fn do_tick(v:&Vec<f64>, tinfo:&mut TradeInfo) {
    
    let s = stg::tick(v, tinfo);
    let tds:Vec<MakeDecision> = s.0;
    let cds:Vec<CancelDecision> = s.1;
    let tsms = (v[0]* 1000.) as i64;

    for td in tds {
        info!("td={:?}", td);
        if td.ttype == "maker".to_string() {
            add_pending(tinfo, &td);
        }
        else if td.ttype == "taker".to_string() {
            add_taking(tinfo, &td);
        }
    }
    
    for cd in cds {
        drop_pending((v[0]*1000.) as i64,tinfo, &cd);
    }
    
    record_pendings(tsms, tinfo)
}

fn do_mm(v:&Vec<f64>, tinfo:&mut TradeInfo) {
    if v[5] == 1. {
        lprocess::process(v, tinfo);
    }
    else if v[5] == 0. {
        tprocess::process(v, tinfo);
    }
    
}

fn do_symbol(symbol:&str) {
    info!("start symbol={}",symbol);
    
    let mut tinfo = TradeInfo::new(symbol.to_string(), &ENGIN_CONF.sids);
    let tick_path = String::from(&ENGIN_CONF.tick_path)+"/"+symbol+".npy";
    
    let start_date = NaiveDate::parse_from_str(&ENGIN_CONF.start_date, "%Y%m%d").unwrap();
    let end_date = NaiveDate::parse_from_str(&ENGIN_CONF.end_date, "%Y%m%d").unwrap();
    
    let reader_tick = File::open(&tick_path).unwrap();
    let ticker_npy = Array2::<f64>::read_npy(reader_tick).unwrap();
    
    let mut iter_tick = ticker_npy.axis_iter(Axis(0));
    
    
    let mut current_date = start_date;
    let mut tick_finished = false;
    let mut mm_finished = false;
    
    while current_date <= end_date {
        if tick_finished {
            info!("finish, exit");
            break
        }
        
        for i in 0..24 {
            let datestr = format!("{:02}", i);
            
        
            let merged_market_path = String::from(&ENGIN_CONF.merged_market_path)+"/"+symbol+"_"+&current_date.format("%Y%m%d").to_string()+"_"+&datestr+".npy";
            if !fs::metadata(merged_market_path.clone()).is_ok() {
                info!("{} read mm {} empty, ignore", current_date.format("%Y%m%d"), &merged_market_path);
                continue;
            }
            info!("{}, read mm merged_market_path={}", current_date.format("%Y%m%d"), &merged_market_path);
            let reader_merged_market = File::open(merged_market_path).unwrap();
            let merged_market_npy = Array2::<f64>::read_npy(reader_merged_market).unwrap();
            let mut iter_mm = merged_market_npy.axis_iter(Axis(0));

            let mut jump_ts:f64 = 0.;


            loop {
                if mm_finished {
                    mm_finished = false;
                    break
                }

                match iter_tick.next() {
                    Some(value) => {
                        let next_tick_ts:f64 = value[0]*1000000.;
                        let curr_ts_s:i64 = value[0] as i64;

                        if curr_ts_s < ENGIN_CONF.start_ts {
                            continue
                        }
                        else {
                            info!("start from ts={}", curr_ts_s);
                        }
                        if curr_ts_s > ENGIN_CONF.end_ts {
                            info!("reach end_ts");
                            tick_finished = true;
                            break
                        }

                        debug!("get tick next ts: {} jump_ts={}", next_tick_ts, jump_ts);
                        if next_tick_ts < jump_ts {
                            debug!("smaller then jump ts: {}, process tick and continue", jump_ts);
                            continue;
                        }


                        loop {
                            match iter_mm.next() {
                                Some(value_mm) => {
                                    let mm_ts = value_mm[0];
                                    debug!("get mm ts={}", mm_ts);
                                    let vec_mm: Vec<f64> = arraybase_to_vec(value_mm);
                                    do_mm(&vec_mm, &mut tinfo);

                                    if mm_ts > next_tick_ts {
                                        debug!("mm_ts={} > tickts={} process last tick...", mm_ts, next_tick_ts);
                                        let vec_tick: Vec<f64> = arraybase_to_vec(value);

                                        do_tick(&vec_tick, &mut tinfo);


                                        debug!("jump to first tick greater than {}",mm_ts);
                                        jump_ts = mm_ts;
                                        break;
                                    }
                                }
                                None => {
                                    info!("mm iter has ended, jump to next date");
                                    mm_finished = true;
                                    break
                                }
                            }

                        }

                    }
                    None => {
                        info!("tick iter has ended");
                        tick_finished = true;
                        break
                    }
                }
            }
        }
        current_date += Duration::days(1);
        info!("add one {}, end={}", current_date, end_date);
    }

}

fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    info!("highres starting..");
        
    for symbol in &ENGIN_CONF.symbols {
        
        do_symbol(symbol);
        clear_pending();
        
    }
        
    for symbol in &ENGIN_CONF.symbols {
        let file_name = ENGIN_CONF.dump_path.to_string()+"/"+symbol+"_orders.csv";
        println!("file_name {:?}",file_name);
        split_csv_file(&file_name,&(symbol.to_string()+"_orders_"),false,10);
    }
    
}