use log::{info};
//use std::error::Error;
use std::fs::File;
use csv::ReaderBuilder;
use std::collections::{HashMap, VecDeque};
use super::super::trade::{TradeInfo, DepthInfo, MakeDecision, CancelDecision};
use crate::gconf::{ENGIN_CONF,NMT_CONF};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::spending::{get_pending_num_from_key, get_pending_num_from_key_bid, get_pending_num_from_key_ask};
use crate::spending::{S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS, PendingItem};
//use ordered_float::OrderedFloat;
use std::time::Duration;
//use crate::symbolinfo;
//use markets::Market;
use crate::symbolinfo::get_market;
use crate::target_sid_open::{add, get};
use parking_lot::RwLock;

use rust_decimal::Decimal;
use std::str::FromStr;
use rust_decimal::prelude::*;

extern crate csv;



//["ts", "p_"+nside+"_"+str(ksm["sid"])+"_"+str(kst["sid"])+"_0", "p_"+nside+"_"+str(ksm["sid"])+"_"+str(kst["sid"])+"_1", "p_"+nside+"_"+str(ksm["sid"])+"_"+str(kst["sid"])+"_2", "p_"+nside+"_"+str(ksm["sid"])+"_"+str(kst["sid"])+"_3"]

lazy_static! {

    
    static ref COLS: HashMap<String, usize> = {
        let mut map = HashMap::new();
        map.insert("ts".to_string(), 0);
        map.insert("p_b_0_1_0".to_string(), 1);
        map.insert("p_b_0_1_1".to_string(), 2);
        map.insert("p_b_0_1_2".to_string(), 3);
        map.insert("p_b_0_1_3".to_string(), 4);        
        map.insert("p_a_0_1_0".to_string(), 5);
        map.insert("p_a_0_1_1".to_string(), 6);
        map.insert("p_a_0_1_2".to_string(), 7);
        map.insert("p_a_0_1_3".to_string(), 8);
        
        map.insert("p_b_0_2_0".to_string(), 9);
        map.insert("p_b_0_2_1".to_string(), 10);
        map.insert("p_b_0_2_2".to_string(), 11);
        map.insert("p_b_0_2_3".to_string(), 12);        
        map.insert("p_a_0_2_0".to_string(), 13);
        map.insert("p_a_0_2_1".to_string(), 14);
        map.insert("p_a_0_2_2".to_string(), 15);
        map.insert("p_a_0_2_3".to_string(), 16);
        
        map.insert("p_b_0_3_0".to_string(), 17);
        map.insert("p_b_0_3_1".to_string(), 18);
        map.insert("p_b_0_3_2".to_string(), 19);
        map.insert("p_b_0_3_3".to_string(), 20);        
        map.insert("p_a_0_3_0".to_string(), 21);
        map.insert("p_a_0_3_1".to_string(), 22);
        map.insert("p_a_0_3_2".to_string(), 23);
        map.insert("p_a_0_3_3".to_string(), 24);
        
        map.insert("p_b_0_4_0".to_string(), 25);
        map.insert("p_b_0_4_1".to_string(), 26);
        map.insert("p_b_0_4_2".to_string(), 27);
        map.insert("p_b_0_4_3".to_string(), 28);        
        map.insert("p_a_0_4_0".to_string(), 29);
        map.insert("p_a_0_4_1".to_string(), 30);
        map.insert("p_a_0_4_2".to_string(), 31);
        map.insert("p_a_0_4_3".to_string(), 32);
        
        
        map.insert("p_b_1_0_0".to_string(), 33);
        map.insert("p_b_1_0_1".to_string(), 34);
        map.insert("p_b_1_0_2".to_string(), 35);
        map.insert("p_b_1_0_3".to_string(), 36);        
        map.insert("p_a_1_0_0".to_string(), 37);
        map.insert("p_a_1_0_1".to_string(), 38);
        map.insert("p_a_1_0_2".to_string(), 39);
        map.insert("p_a_1_0_3".to_string(), 40);
        
        map.insert("p_b_1_2_0".to_string(), 41);
        map.insert("p_b_1_2_1".to_string(), 42);
        map.insert("p_b_1_2_2".to_string(), 43);
        map.insert("p_b_1_2_3".to_string(), 44);        
        map.insert("p_a_1_2_0".to_string(), 45);
        map.insert("p_a_1_2_1".to_string(), 46);
        map.insert("p_a_1_2_2".to_string(), 47);
        map.insert("p_a_1_2_3".to_string(), 48);
        
        map.insert("p_b_1_3_0".to_string(), 49);
        map.insert("p_b_1_3_1".to_string(), 50);
        map.insert("p_b_1_3_2".to_string(), 51);
        map.insert("p_b_1_3_3".to_string(), 52);        
        map.insert("p_a_1_3_0".to_string(), 53);
        map.insert("p_a_1_3_1".to_string(), 54);
        map.insert("p_a_1_3_2".to_string(), 55);
        map.insert("p_a_1_3_3".to_string(), 56);
        
        map.insert("p_b_1_4_0".to_string(), 57);
        map.insert("p_b_1_4_1".to_string(), 58);
        map.insert("p_b_1_4_2".to_string(), 59);
        map.insert("p_b_1_4_3".to_string(), 60);        
        map.insert("p_a_1_4_0".to_string(), 61);
        map.insert("p_a_1_4_1".to_string(), 62);
        map.insert("p_a_1_4_2".to_string(), 63);
        map.insert("p_a_1_4_3".to_string(), 64);
        
        
        map.insert("p_b_2_0_0".to_string(), 65);
        map.insert("p_b_2_0_1".to_string(), 66);
        map.insert("p_b_2_0_2".to_string(), 67);
        map.insert("p_b_2_0_3".to_string(), 68);        
        map.insert("p_a_2_0_0".to_string(), 69);
        map.insert("p_a_2_0_1".to_string(), 70);
        map.insert("p_a_2_0_2".to_string(), 71);
        map.insert("p_a_2_0_3".to_string(), 72);
        
        map.insert("p_b_2_1_0".to_string(), 73);
        map.insert("p_b_2_1_1".to_string(), 74);
        map.insert("p_b_2_1_2".to_string(), 75);
        map.insert("p_b_2_1_3".to_string(), 76);        
        map.insert("p_a_2_1_0".to_string(), 77);
        map.insert("p_a_2_1_1".to_string(), 78);
        map.insert("p_a_2_1_2".to_string(), 79);
        map.insert("p_a_2_1_3".to_string(), 80);
        
        map.insert("p_b_2_3_0".to_string(), 81);
        map.insert("p_b_2_3_1".to_string(), 82);
        map.insert("p_b_2_3_2".to_string(), 83);
        map.insert("p_b_2_3_3".to_string(), 84);        
        map.insert("p_a_2_3_0".to_string(), 85);
        map.insert("p_a_2_3_1".to_string(), 86);
        map.insert("p_a_2_3_2".to_string(), 87);
        map.insert("p_a_2_3_3".to_string(), 88);
        
        map.insert("p_b_2_4_0".to_string(), 89);
        map.insert("p_b_2_4_1".to_string(), 90);
        map.insert("p_b_2_4_2".to_string(), 91);
        map.insert("p_b_2_4_3".to_string(), 92);        
        map.insert("p_a_2_4_0".to_string(), 93);
        map.insert("p_a_2_4_1".to_string(), 94);
        map.insert("p_a_2_4_2".to_string(), 95);
        map.insert("p_a_2_4_3".to_string(), 96);
        
        
        map.insert("p_b_3_0_0".to_string(), 97);
        map.insert("p_b_3_0_1".to_string(), 98);
        map.insert("p_b_3_0_2".to_string(), 99);
        map.insert("p_b_3_0_3".to_string(), 100);        
        map.insert("p_a_3_0_0".to_string(), 101);
        map.insert("p_a_3_0_1".to_string(), 102);
        map.insert("p_a_3_0_2".to_string(), 103);
        map.insert("p_a_3_0_3".to_string(), 104);
        
        map.insert("p_b_3_1_0".to_string(), 105);
        map.insert("p_b_3_1_1".to_string(), 106);
        map.insert("p_b_3_1_2".to_string(), 107);
        map.insert("p_b_3_1_3".to_string(), 108);        
        map.insert("p_a_3_1_0".to_string(), 109);
        map.insert("p_a_3_1_1".to_string(), 110);
        map.insert("p_a_3_1_2".to_string(), 111);
        map.insert("p_a_3_1_3".to_string(), 112);
        
        map.insert("p_b_3_2_0".to_string(), 113);
        map.insert("p_b_3_2_1".to_string(), 114);
        map.insert("p_b_3_2_2".to_string(), 115);
        map.insert("p_b_3_2_3".to_string(), 116);        
        map.insert("p_a_3_2_0".to_string(), 117);
        map.insert("p_a_3_2_1".to_string(), 118);
        map.insert("p_a_3_2_2".to_string(), 119);
        map.insert("p_a_3_2_3".to_string(), 120);
        
        map.insert("p_b_3_4_0".to_string(), 121);
        map.insert("p_b_3_4_1".to_string(), 122);
        map.insert("p_b_3_4_2".to_string(), 123);
        map.insert("p_b_3_4_3".to_string(), 124);        
        map.insert("p_a_3_4_0".to_string(), 125);
        map.insert("p_a_3_4_1".to_string(), 126);
        map.insert("p_a_3_4_2".to_string(), 127);
        map.insert("p_a_3_4_3".to_string(), 128);

        
        map.insert("p_b_4_0_0".to_string(), 129);
        map.insert("p_b_4_0_1".to_string(), 130);
        map.insert("p_b_4_0_2".to_string(), 131);
        map.insert("p_b_4_0_3".to_string(), 132);        
        map.insert("p_a_4_0_0".to_string(), 133);
        map.insert("p_a_4_0_1".to_string(), 134);
        map.insert("p_a_4_0_2".to_string(), 135);
        map.insert("p_a_4_0_3".to_string(), 136);
    
        map.insert("p_b_4_1_0".to_string(), 137);
        map.insert("p_b_4_1_1".to_string(), 138);
        map.insert("p_b_4_1_2".to_string(), 139);
        map.insert("p_b_4_1_3".to_string(), 140);        
        map.insert("p_a_4_1_0".to_string(), 141);
        map.insert("p_a_4_1_1".to_string(), 142);
        map.insert("p_a_4_1_2".to_string(), 143);
        map.insert("p_a_4_1_3".to_string(), 144);
        
        map.insert("p_b_4_2_0".to_string(), 145);
        map.insert("p_b_4_2_1".to_string(), 146);
        map.insert("p_b_4_2_2".to_string(), 147);
        map.insert("p_b_4_2_3".to_string(), 148);        
        map.insert("p_a_4_2_0".to_string(), 149);
        map.insert("p_a_4_2_1".to_string(), 150);
        map.insert("p_a_4_2_2".to_string(), 151);
        map.insert("p_a_4_2_3".to_string(), 152);
        
        map.insert("p_b_4_3_0".to_string(), 153);
        map.insert("p_b_4_3_1".to_string(), 154);
        map.insert("p_b_4_3_2".to_string(), 155);
        map.insert("p_b_4_3_3".to_string(), 156);        
        map.insert("p_a_4_3_0".to_string(), 157);
        map.insert("p_a_4_3_1".to_string(), 158);
        map.insert("p_a_4_3_2".to_string(), 159);
        map.insert("p_a_4_3_3".to_string(), 160);        

        map
    };
    
    static ref COID_INC: AtomicUsize = AtomicUsize::new(0);
    
    
    //taker pos as pair pos
    static ref PAIRPOS: HashMap<String, RwLock<f64>> = {
        let mut map = HashMap::new();
        
        for msid in ENGIN_CONF.vsids.iter() {
            for tsid in ENGIN_CONF.vsids.iter() {
                if msid == tsid {
                    continue
                }
                map.insert(msid.to_string()+"_"+&tsid.to_string(), RwLock::new(0.));
            }

        }
        map
    };
    
    static ref FQUANTILE: HashMap<String, RwLock<VecDeque<f64>>> = {
        let mut map = HashMap::new();
        let rbs = "0,0.0001,0.0002,0.0003";
        for symbol in ENGIN_CONF.symbols.iter() {
            for msid in ENGIN_CONF.vsids.iter() {
                for tsid in ENGIN_CONF.vsids.iter() {
                    if msid == tsid {
                        continue
                    }
                    for rbstr in rbs.split(',') {
                        let key = symbol.to_string()+"_"+&msid.to_string()+"_"+&tsid.to_string()+"_"+rbstr;
                        map.insert(key, RwLock::new(VecDeque::new()));
                    }
                    
                }
            }
        }
        map
    };
    
    
    static ref RMEAN: HashMap<String, RwLock<f64>> = {
        let mut map = HashMap::new();
        let rbs = "0,0.0001,0.0002,0.0003";

        for symbol in ENGIN_CONF.symbols.iter() {
            for msid in ENGIN_CONF.vsids.iter() {
                for tsid in ENGIN_CONF.vsids.iter() {
                    if msid == tsid {
                        continue
                    }
                    
                    for rbstr in rbs.split(',') {
                        let key = symbol.to_string()+"_"+&msid.to_string()+"_"+&tsid.to_string()+"_"+rbstr;
                        map.insert(key, RwLock::new(-100.));
                        
                    }
                }
            }
        }
        map
    };

}

#[derive(Debug)]
pub struct GConfItem {
    pub tf:f64,
    pub rx:f64
}

pub fn init_residule_mean() {
    let file = File::open(NMT_CONF.stat_path.to_string()+"/1.csv").unwrap();
    let mut csv_reader = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);
    let headers = csv_reader.headers().unwrap().clone();
    let headers: Vec<String> = headers.iter()
        .map(|s| s.to_string())
        .collect();


    for result in csv_reader.records() {
        let record = result.unwrap();        
        if record[0] == "residual_mean".to_string() {      
            for (i, field) in record.iter().enumerate() {
                if headers[i] != "".to_string() {
                    let rm_f64:f64 = field.parse().unwrap();
                    //info!("{:?}: {}", headers[i], field);
                    if RMEAN.contains_key(&headers[i]) {
                    
                        if let Some(mut rm) = RMEAN[&headers[i]].try_write_for(Duration::from_secs(1)) {
                            //*rm = rm_f64;
                            *rm = 0.;
                            //info!("{:?}: {}", headers[i], field);
                        }
                    }
                }
            }
        }
    }
}

pub fn init_quantile() {
    let file = File::open(NMT_CONF.stat_path.to_string()+"/1_quantile.csv").unwrap();
    let mut csv_reader = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);
    let headers = csv_reader.headers().unwrap().clone();
    let headers: Vec<String> = headers.iter()
        .map(|s| s.to_string())
        .collect();


    for result in csv_reader.records() {
        let record = result.unwrap();        

        for (i, field) in record.iter().enumerate() {
            //info!("{:?}: {}", headers[i], field);
            let rm_f64:f64 = field.parse().unwrap();
            if FQUANTILE.contains_key(&headers[i]) {
                if let Some(mut vqueue) = FQUANTILE[&headers[i]].try_write_for(Duration::from_secs(1)) {
                    vqueue.push_back(rm_f64);
                }
            }
        }
    }
//     let k = "XRPUSDT_2_1_0".to_string();
//     let vqs = FQUANTILE[&k].read();

    
//     for fqitem in vqs.iter() {
//         info!("{}", fqitem);
//     }
}

pub fn cb_init() {
    //read stat
    
    init_residule_mean();
    init_quantile();

}

pub fn cb_filled(tinfo:&mut TradeInfo, pitem:&PendingItem, filled_amount:f64) {
    //pitem.split(',').collect();
    let ps: Vec<&str> = pitem.from_key.split("_").collect();
    let pair = String::from(ps[1]) + "_" + ps[2];
    info!("cb filled, side={} famount={} pair={}  ", pitem.side, filled_amount, pair);
    
    
    
    if let Some(mut pos) = PAIRPOS[&pair].try_write_for(Duration::from_secs(1)) {
        info!("pair={} currpos={} side={}", pair, *pos, pitem.side);
        if pitem.side == "bid" {
            (*pos)+=filled_amount;
        }
        else if pitem.side == "ask" {
            (*pos)-=filled_amount;
        }
        else {
            panic!("no side");
        }
    }
    
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

pub fn parse_gconf(gconfstr_open:&str, gconfstr_close:&str)->(HashMap<i32, Vec<GConfItem>>, HashMap<i32, Vec<GConfItem>>)  {
    
    //open
    let mut hmap_open:HashMap<i32, Vec<GConfItem>> = HashMap::new();
    let mut hmap_close:HashMap<i32, Vec<GConfItem>> = HashMap::new();
    
    let parts_open: Vec<&str> = gconfstr_open.split(';').collect();
    let parts_close: Vec<&str> = gconfstr_close.split(';').collect();


    
    for pitem in parts_open {
        
        let ps: Vec<&str> = pitem.split(',').collect();
        let rb:i32 = String::from(ps[0]).parse::<i32>().unwrap();
        //
        let gc = GConfItem{tf:(String::from(ps[1]).parse::<f64>().unwrap() as f64), rx:(String::from(ps[2]).parse::<f64>().unwrap())};
      
        
        if let Some(vitems) = hmap_open.get_mut(&rb) {
            vitems.push(gc);
        }
        else {
            let mut hv:Vec<GConfItem> = Vec::new();
            hv.push(gc);
            hmap_open.insert(rb, hv);
        }
    }
    
    for pitem in parts_close {
        
        let ps: Vec<&str> = pitem.split(',').collect();
        let rb:i32 = String::from(ps[0]).parse::<i32>().unwrap();
        //
        let gc = GConfItem{tf:(String::from(ps[1]).parse::<f64>().unwrap() as f64), rx:(String::from(ps[2]).parse::<f64>().unwrap())};
      
        
        if let Some(vitems) = hmap_close.get_mut(&rb) {
            vitems.push(gc);
        }
        else {
            let mut hv:Vec<GConfItem> = Vec::new();
            hv.push(gc);
            hmap_close.insert(rb, hv);
        }
    }
    
    return (hmap_open,hmap_close);
}


pub fn is_close(tinfo:&TradeInfo, f:f64, pair:String, fee:f64, side:&str, rangekey:i32)->bool {
    let mut key = "".to_string();
    let range = rangekey as f64 * 0.0001;
    let pos = (*PAIRPOS[&pair].read());
    
    if rangekey == 0 {
        key = format!("{}_{}_0", tinfo.symbol, pair);
    }
    else {
        key = format!("{}_{}_{:.4}", tinfo.symbol, pair, range);
    }
    let rmean = (*RMEAN[&key].read());

    let vqs = FQUANTILE[&key].read();
    if vqs.len() == 0 {
        return false;
    }
    
    let mut leverage_num_from = 5;
    
    if side == "bid" && pos < 0. {
        for fqitem in vqs.iter() {          
            info!("is_close bid f={} rmean={} fqitem={} fee={} pos={} leveragex={}", f, rmean, fqitem, fee, pos, leverage_num_from as f64 * NMT_CONF.max_pos);
            info!("fc={} fthr={} ", f + rmean, -(fqitem - NMT_CONF.profit_range) + fee + NMT_CONF.thr_add_close);

            if f + rmean > -(fqitem - NMT_CONF.profit_range) + fee + NMT_CONF.thr_add_close && pos < -leverage_num_from as f64 * NMT_CONF.max_pos {            
                info!("is_close pass bid");
                return true;
            }
            leverage_num_from-=1;
        }
    }
    else if side == "ask" && pos > 0. {
        for fqitem in vqs.iter() {       
            info!("is_close ask f={} rmean={} fqitem={} fee={} pos={} leveragex={}", f, rmean, fqitem, fee, pos, leverage_num_from as f64 * NMT_CONF.max_pos);
            info!("fc={} fthr={} ", f + rmean, -(fqitem - NMT_CONF.profit_range) + fee + NMT_CONF.thr_add_close + fee + NMT_CONF.thr_add_close);

            if f + rmean > -(fqitem - NMT_CONF.profit_range) + fee + NMT_CONF.thr_add_close && pos > leverage_num_from as f64 * NMT_CONF.max_pos { 
                info!("is_close pass ask");
                return true;
            }
            leverage_num_from-=1;
        }
    }
    else {
        if side != "bid" && side != "ask" {
            panic!("mom such side");
        }

    }
    return false
    
}

pub fn is_open(tinfo:&TradeInfo, f:f64, pair:String, fee:f64, side:&str, rangekey:i32)->bool {
    let mut key = "".to_string();
    let range = rangekey as f64 * 0.0001;
    let pos = (*PAIRPOS[&pair].read());
    
    if rangekey == 0 {
        key = format!("{}_{}_0", tinfo.symbol, pair);
    }
    else {
        key = format!("{}_{}_{:.4}", tinfo.symbol, pair, range);
    }
    info!("key={}", key);
    let rmean = (*RMEAN[&key].read());
    let vqs = FQUANTILE[&key].read();
    if vqs.len() == 0 {
        return false;
    }
    
    let mut leverage_num_from = 5;
    if side == "bid" {
        for fqitem in vqs.iter() {            
            info!("is_open bid f={} rmean={} fqitem={} fee={} pos={} leveragex={}", f, rmean, fqitem, fee, pos, leverage_num_from as f64 * NMT_CONF.max_pos);
            info!("fc={} fthr={} ", f + rmean, fqitem + fee + NMT_CONF.thr_add_open);
 
            if f + rmean > fqitem + fee + NMT_CONF.thr_add_open && pos < leverage_num_from as f64 * NMT_CONF.max_pos {            
                
                info!("pass isopen bid");
                return true;
            }
            leverage_num_from-=1;
        }
    }
    else if side == "ask" {
        for fqitem in vqs.iter() {     
            info!("is_open ask f={} rmean={} fqitem={} fee={} pos={} leveragex={}", f, rmean, fqitem, fee, pos, leverage_num_from as f64 * NMT_CONF.max_pos);
            info!("fc={} fthr={} ", f + rmean, fqitem + fee + NMT_CONF.thr_add_open);

            if f + rmean > fqitem + fee + NMT_CONF.thr_add_open && pos > -leverage_num_from as f64 * NMT_CONF.max_pos { 
                info!("pass isopen ask");
                return true;
            }
            leverage_num_from-=1;
        }
    }
    else {
        panic!("mom such side");

    }
    return false
    
}

pub fn make(t:&Vec<f64>, tinfo:&mut TradeInfo) -> Vec<MakeDecision>{
    let mut tds:Vec<MakeDecision> = Vec::new();
    let (gconfmap_open, gconfmap_close) = parse_gconf(&NMT_CONF.gconf_open, &NMT_CONF.gconf_close);
    
    
    info!("gconfmap_open={:?}", gconfmap_open);
    info!("gconfmap_close={:?}", gconfmap_close);
  
    
    
    //taker - 记录 pos 仓位 
    for sid in ENGIN_CONF.vsids.iter() {
        let e_m = &ENGIN_CONF.sids[sid];
        let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
        let dinfo_m:&DepthInfo = tinfo.depths.get(&sid).unwrap();
        let pos_m:f64 = *tinfo.pos.get(&sid).unwrap();
        
        let mut contract_value_m = 1.;
        if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
            contract_value_m = market_m.contract_value.unwrap();
        }
        
        let mut amount_in_target_sid = get(sid) / contract_value_m;
        let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;
        let amount_hand_token = NMT_CONF.amountu / contract_value_m / mid_m;
        
        if amount_in_target_sid < 0. {
            //buy , max amount_hand_token
            let amount_in_target_sid_taker = (-amount_in_target_sid).min(amount_hand_token * 3.);
            
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "buy_t_".to_string()+&sid.to_string()+"_"+&curr_cid.to_string();
            let pairkey = sid.to_string();
            
            tds.push(MakeDecision{
                create_ts:t[0] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:0, 
                side:"buy".to_string(), 
                sid:*sid, 
                ttype:"taker".to_string(), 
                price:0., 
                amount:amount_in_target_sid_taker, 
                from_key:pairkey.to_string(),target_sid:-1});
            
            
        } else if amount_in_target_sid > 0. {
            //sell
            let amount_in_target_sid_taker = (amount_in_target_sid).min(amount_hand_token * 3.);
            
            let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
            let client_order_id = "sell_t_".to_string()+&sid.to_string()+"_"+&curr_cid.to_string();
            let pairkey = sid.to_string();
            
            tds.push(MakeDecision{
                create_ts:t[0] as i64,
                client_order_id:client_order_id, 
                max_order_keep_s:0, 
                side:"sell".to_string(), 
                sid:*sid, 
                ttype:"taker".to_string(), 
                price:0., 
                amount:amount_in_target_sid_taker, 
                from_key:pairkey.to_string(),target_sid:-1});
        }
  
    }
    
    //maker
    for sidm in ENGIN_CONF.vsids.iter() {
        let e_m = &ENGIN_CONF.sids[sidm];
        let market_m = get_market().get((e_m["exchange"].to_string()+":"+&e_m["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
        let dinfo_m:&DepthInfo = tinfo.depths.get(&sidm).unwrap();
        let pos_m:f64 = *tinfo.pos.get(&sidm).unwrap();
        
        let mut contract_value_m = 1.;
        if e_m["etype"] == "swap"  && e_m["exchange"] == "okx" {
            contract_value_m = market_m.contract_value.unwrap();
        }
        
        if !dinfo_m.is_finish_snap {
            continue
        }
        let mid_m:f64 = (dinfo_m.bid1 + dinfo_m.ask1) / 2.;
        let upos_m:f64 = pos_m * contract_value_m * mid_m;
        let amount_hand_token = NMT_CONF.amountu / contract_value_m / mid_m;
        
        
        for sidt in ENGIN_CONF.vsids.iter() {            
            if sidm == sidt {
                continue
            }
            
            let e_t = &ENGIN_CONF.sids[sidt];
            let market_t = get_market().get((e_t["exchange"].to_string()+":"+&e_t["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
            let dinfo_t:&DepthInfo = tinfo.depths.get(&sidt).unwrap();
            let pos_t:f64 = *tinfo.pos.get(&sidt).unwrap();

            let mut contract_value_t = 1.;
            if e_t["etype"] == "swap"  && e_t["exchange"] == "okx" {
                contract_value_t = market_t.contract_value.unwrap();
            }

            if !dinfo_t.is_finish_snap {
                continue
            }
            let mid_t:f64 = (dinfo_t.bid1 + dinfo_t.ask1) / 2.;
            let upos_t:f64 = pos_t * contract_value_t * mid_t;
            let fee = &NMT_CONF.fees[sidm];
            let feem = fee["m"];
            
            //bid push order
            for rangekey in vec![0,1,2,3] {
                let fee = &NMT_CONF.fees[sidt];
                let feet = fee["t"];
            
                let fkey = format!("p_b_{}_{}_{}", sidm, sidt, rangekey);
                let f = t[*COLS.get(&fkey).unwrap()] as f64;
                
                info!("bid [{} {} {}], f={}", sidm, sidt, fkey, f);
                let total_fee = feem + feet;
                
                //是否平仓,1.pair-pos 2.all-pos limit
                //bid
                let close = is_close(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "bid", rangekey);
             
                if close && upos_m < NMT_CONF.max_pos * 5. && upos_t > -NMT_CONF.max_pos * 5. {
                
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "bid_m_".to_string()+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string()+"_"+&curr_cid.to_string();
                    let pairkey = "b".to_string()+"_"+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string();
                    let rb_bid = (rangekey as f64) * 0.0001;
                    let bid_price:f64 = dinfo_m.bid1 * (1. - rb_bid);


                    let num_pending = get_pending_num_from_key_bid(*sidm, &pairkey);
                    if num_pending == 0 {      
                        tds.push(MakeDecision{
                            create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                            client_order_id:client_order_id, 
                            max_order_keep_s:0, side:"buy".to_string(), 
                            sid:*sidm, 
                            ttype:"maker".to_string(), 
                            price:bid_price, 
                            amount:amount_hand_token, 
                            from_key:pairkey.to_string(),
                            target_sid:*sidt}); 
                    }
                }
                else {
                    //是否开仓,1.pair-pos 2.all-pos limit

                    let open = is_open(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "bid", rangekey);

                    if open && upos_m < NMT_CONF.max_pos * 5. && upos_t > -NMT_CONF.max_pos * 5. {
                        let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                        let client_order_id = "bid_m_".to_string()+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string()+"_"+&curr_cid.to_string();
                        let pairkey = "b".to_string()+"_"+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string();
                        let rb_bid = (rangekey as f64) * 0.0001;
                        let bid_price:f64 = dinfo_m.bid1 * (1. - rb_bid);


                        let num_pending = get_pending_num_from_key_bid(*sidm, &pairkey);
                        if num_pending == 0 {      
                            tds.push(MakeDecision{
                                create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                                client_order_id:client_order_id, 
                                max_order_keep_s:0, side:"buy".to_string(), 
                                sid:*sidm, 
                                ttype:"maker".to_string(), 
                                price:bid_price, 
                                amount:amount_hand_token, 
                                from_key:pairkey.to_string(),
                                target_sid:*sidt}); 
                        }

                    }
                }
            }
            //ask push order
            for rangekey in vec![0,1,2,3] {
                let fee = &NMT_CONF.fees[sidt];
                let feet = fee["t"];
            
                
                let fkey = format!("p_a_{}_{}_{}", sidm, sidt, rangekey);
                let f = t[*COLS.get(&fkey).unwrap()] as f64;
                
                
                info!("ask [{} {} {}], f={}", sidm, sidt, fkey, f);
                let total_fee = feem + feet;
                
                //是否平仓,1.pair-pos 2.all-pos limit
                //ask
                let close = is_close(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "ask", rangekey);

                if close && upos_m > -NMT_CONF.max_pos * 5. && upos_t < NMT_CONF.max_pos * 5. {
                
                    let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                    let client_order_id = "ask_m_".to_string()+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string()+"_"+&curr_cid.to_string();
                    let pairkey = "a".to_string()+"_"+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string();
                    let rb_bid = (rangekey as f64) * 0.0001;
                    let ask_price:f64 = dinfo_m.ask1 * (1. + rb_bid);

                    let num_pending = get_pending_num_from_key_ask(*sidm, &pairkey);
                    if num_pending == 0 {      
                        tds.push(MakeDecision{
                            create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                            client_order_id:client_order_id, 
                            max_order_keep_s:0, side:"sell".to_string(), 
                            sid:*sidm, 
                            ttype:"maker".to_string(), 
                            price:ask_price, 
                            amount:amount_hand_token, 
                            from_key:pairkey.to_string(),
                            target_sid:*sidt}); 
                    }
                }
                else {
                    //是否开仓,1.pair-pos 2.all-pos limit

                    let open = is_open(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "ask", rangekey);

                    if open && upos_m > -NMT_CONF.max_pos * 5. && upos_t < NMT_CONF.max_pos * 5. {
                        let curr_cid = COID_INC.fetch_add(1, Ordering::Relaxed);
                        let client_order_id = "ask_m_".to_string()+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string()+"_"+&curr_cid.to_string();
                        let pairkey = "a".to_string()+"_"+&sidm.to_string()+"_"+&sidt.to_string()+"_"+&rangekey.to_string();
                        let rb_bid = (rangekey as f64) * 0.0001;
                        let ask_price:f64 = dinfo_m.ask1 * (1. + rb_bid);


                        let num_pending = get_pending_num_from_key_ask(*sidm, &pairkey);
                        if num_pending == 0 {      
                            tds.push(MakeDecision{
                                create_ts:t[*COLS.get("ts").unwrap()] as i64, 
                                client_order_id:client_order_id, 
                                max_order_keep_s:0, side:"sell".to_string(), 
                                sid:*sidm, 
                                ttype:"maker".to_string(), 
                                price:ask_price, 
                                amount:amount_hand_token, 
                                from_key:pairkey.to_string(),
                                target_sid:*sidt}); 
                        }

                    }
                }
            }
    
        }
    }


    return tds;
}


pub fn is_cancel(t:&Vec<f64>, tinfo:&mut TradeInfo, pitem:&PendingItem, sid:&i32) -> bool{
    //计算信号
    //1. 超时cancel
    //2. 信号变化
    
    let (gconfmap_open, gconfmap_close) = parse_gconf(&NMT_CONF.gconf_open, &NMT_CONF.gconf_close);

    
    if (t[0]*1000.) as i64 - pitem.create_ts > (NMT_CONF.max_order_keep_s*1000) as i64 {
        info!("pending last too long o={} c={}", t[0] as i64, pitem.create_ts);
        return true
    }
    
    if pitem.side == "bid" {
        let parts: Vec<&str> = pitem.from_key.split('_').collect();
        let sidm = String::from(parts[1]).parse::<i32>().unwrap();
        let sidt = String::from(parts[2]).parse::<i32>().unwrap();
        let rangekey = String::from(parts[3]).parse::<i32>().unwrap();
        
        let feem_set = &NMT_CONF.fees[&sidm];
        let feem = feem_set["m"];
        
        let feet_set = &NMT_CONF.fees[&sidt];
        let feet = feet_set["t"];
        
        //get current sig
        let fkey = format!("p_b_{}_{}_{}", sidm, sidt, rangekey);
        let f = t[*COLS.get(&fkey).unwrap()] as f64;
        let total_fee = feem + feet;
        
        //check if the signal still matches
        let close = is_close(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "bid", rangekey);
        let open = is_open(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "bid", rangekey);

        
        if !close && !open {
            info!("sigloss bid, id={}", pitem.client_order_id);
            
            return true
        }
        
    } else if pitem.side == "ask" {
        let parts: Vec<&str> = pitem.from_key.split('_').collect();
        let sidm = String::from(parts[1]).parse::<i32>().unwrap();
        let sidt = String::from(parts[2]).parse::<i32>().unwrap();
        let rangekey = String::from(parts[3]).parse::<i32>().unwrap();
        
        let feem_set = &NMT_CONF.fees[&sidm];
        let feem = feem_set["m"];
        
        let feet_set = &NMT_CONF.fees[&sidt];
        let feet = feet_set["t"];
        
        //get current sig
        let fkey = format!("p_a_{}_{}_{}", sidm, sidt, rangekey);
        let f = t[*COLS.get(&fkey).unwrap()] as f64;
        let total_fee = feem + feet;
        
        //check if the signal still matches
        let close = is_close(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "ask", rangekey);
        let open = is_open(tinfo, f, sidm.to_string() + "_" + &sidt.to_string(), total_fee, "ask", rangekey);
        if !close && !open {
            info!("sigloss ask, id={}", pitem.client_order_id);

            
            return true
        }
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

