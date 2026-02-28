use crate::gconf::ENGIN_CONF;
use crate::record::{write_to_csv, RecordDumpItem, TSRecordItem};
use crate::stg::cb_finished;
use crate::symbolinfo::get_market;
use crate::trade::{CancelDecision, DepthInfo, MakeDecision, TradeInfo};
use lazy_static::lazy_static;
use log::info;
use ordered_float::OrderedFloat;
use parking_lot::RwLock;
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

lazy_static! {
    pub static ref S_PENDING_PRICEKEY_BIDS: HashMap<i32, RwLock<PendingOrdersInPos>> = {
        let mut map = HashMap::new();

        for sid in ENGIN_CONF.vsids.iter() {
            let tu = PendingOrdersInPos {
                phash: HashMap::new(),
            };
            map.insert(*sid, RwLock::new(tu));
        }
        map
    };
    pub static ref S_PENDING_PRICEKEY_ASKS: HashMap<i32, RwLock<PendingOrdersInPos>> = {
        let mut map = HashMap::new();
        for sid in ENGIN_CONF.vsids.iter() {
            let tu = PendingOrdersInPos {
                phash: HashMap::new(),
            };
            map.insert(*sid, RwLock::new(tu));
        }
        map
    };
}

pub fn clear_pending() {
    for sid in ENGIN_CONF.vsids.iter() {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(Duration::from_secs(1))
        {
            porders.phash.clear();
        }

        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1))
        {
            porders.phash.clear();
        }
    }
}

pub fn get_pending_num_dup_key_bid(sid: i32, dup_key: &str) -> i32 {
    let mut n_bids: i32 = 0;

    let porders = S_PENDING_PRICEKEY_BIDS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            if v.dup_key == dup_key {
                n_bids += 1
            }
        }
    }

    return n_bids;
}

pub fn get_pending_num_dup_key_ask(sid: i32, dup_key: &str) -> i32 {
    let mut n_asks: i32 = 0;

    let porders = S_PENDING_PRICEKEY_ASKS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            if v.dup_key == dup_key {
                n_asks += 1
            }
        }
    }

    return n_asks;
}

pub fn get_pending_num_from_key_bid(sid: i32, from_key: &str) -> i32 {
    let mut n_bids: i32 = 0;

    let porders = S_PENDING_PRICEKEY_BIDS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            //info!("v={:?}", v);
            if v.from_key == from_key {
                n_bids += 1
            }
        }
    }

    return n_bids;
}

pub fn get_pending_num_from_key_ask(sid: i32, from_key: &str) -> i32 {
    let mut n_asks: i32 = 0;

    let porders = S_PENDING_PRICEKEY_ASKS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            if v.from_key == from_key {
                n_asks += 1
            }
        }
    }

    return n_asks;
}

pub fn get_pending_num_from_key(sid: i32, from_key: &str) -> (i32, i32) {
    let mut n_bids: i32 = 0;
    let mut n_asks: i32 = 0;

    let porders = S_PENDING_PRICEKEY_BIDS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            if v.from_key == from_key {
                n_bids += 1
            }
        }
    }

    let porders = S_PENDING_PRICEKEY_ASKS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            if v.from_key == from_key {
                n_asks += 1
            }
        }
    }

    return (n_bids, n_asks);
}

pub fn get_pending_num_from_sid(sid: i32) -> (i32, i32) {
    let mut n_bids: i32 = 0;
    let mut n_asks: i32 = 0;

    let porders = S_PENDING_PRICEKEY_BIDS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            n_bids += 1
        }
    }

    let porders = S_PENDING_PRICEKEY_ASKS[&sid].read();
    for (_price, vpitem) in porders.phash.iter() {
        for v in vpitem {
            n_asks += 1
        }
    }

    return (n_bids, n_asks);
}

fn adjust_price(price: f64, tick_size: f64, tick_size_keep: u32, is_buy_order: bool) -> f64 {
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

fn count_decimal_places(num: f64) -> usize {
    let num_str = num.to_string();
    if let Some(decimal_index) = num_str.find('.') {
        num_str.len() - decimal_index - 1
    } else {
        0
    }
}

pub fn drop_pending(ts: i64, tinfo: &mut TradeInfo, cds: &CancelDecision) {
    let e = &ENGIN_CONF.sids[&cds.sid];
    let market = get_market()
        .get((e["exchange"].to_string() + ":" + &e["etype"] + ":" + &tinfo.symbol_std).as_str())
        .unwrap();

    let mut contract_value = 1.;
    if e["etype"] == "swap" && e["exchange"] == "okx" {
        contract_value = market.contract_value.unwrap();
    }
    let sid = cds.sid;

    if &cds.side == "bid" {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&cds.sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(cds.price)) {
                Some(vs) => {
                    let mut idx2del: usize = 0;
                    for v in &mut *vs {
                        if &v.client_order_id == &cds.client_order_id {
                            //记录cancel
                            // if v.amount != v.amount_init {
                            //     let update_ts_ms = ts as i64;
                            //     let rd:RecordDumpItem = RecordDumpItem{create_ts:v.create_ts,update_ts:update_ts_ms,client_order_id:v.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), ttype:"maker".to_string(), sid:sid, side:"buy".to_string(), price:v.price, amount_init:v.amount_init*contract_value, amount_update:v.amount*contract_value, status:"canceled".to_string()};
                            //     write_to_csv(&rd);
                            // }
                            info!("cancel cb_finished, v={:?}", v);

                            cb_finished(ts, tinfo, v, true);
                            // cb_finished((ts as i64)/1000, tinfo, v, true);

                            break;
                        }
                        idx2del += 1;
                    }
                    vs.remove(idx2del);

                    if vs.len() == 0 {
                        porders.phash.remove(&OrderedFloat(cds.price));
                    }
                    info!("client_order_id={} pending droped", cds.client_order_id);
                }
                None => {
                    panic!("cannot find porder");
                }
            }
        }
    } else if &cds.side == "ask" {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&cds.sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(cds.price)) {
                Some(vs) => {
                    let mut idx2del: usize = 0;
                    for v in &mut *vs {
                        if &v.client_order_id == &cds.client_order_id {
                            // if v.amount != v.amount_init {
                            //     let update_ts_ms = ts  as i64;
                            //     let rd:RecordDumpItem = RecordDumpItem{create_ts:v.create_ts,update_ts:update_ts_ms,client_order_id:v.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), ttype:"maker".to_string(), sid:sid, side:"sell".to_string(), price:v.price, amount_init:v.amount_init*contract_value, amount_update:v.amount*contract_value, status:"canceled".to_string()};
                            //     write_to_csv(&rd);
                            // }
                            cb_finished(ts, tinfo, v, true);
                            // cb_finished((ts as i64)/1000, tinfo, v, true);
                            break;
                        }
                        idx2del += 1;
                    }
                    vs.remove(idx2del);

                    if vs.len() == 0 {
                        porders.phash.remove(&OrderedFloat(cds.price));
                    }
                    info!("client_order_id={} pending droped", cds.client_order_id);
                }
                None => {
                    panic!("cannot find porder");
                }
            }
        }
    } else {
        panic!("side={}", cds.side);
    }
}

pub fn add_pending(tinfo: &mut TradeInfo, tds: &MakeDecision) {
    let e = &ENGIN_CONF.sids[&tds.sid];
    let market = get_market()
        .get((e["exchange"].to_string() + ":" + &e["etype"] + ":" + &tinfo.symbol_std).as_str())
        .unwrap();
    let tick_size = market.precision.tick_size;
    let tick_size_keep = count_decimal_places(tick_size) as u32;
    let rd = RoundingStrategy::ToZero;
    let ru = RoundingStrategy::AwayFromZero;

    let dinfo: &mut DepthInfo = tinfo.depths.get_mut(&tds.sid).unwrap();

    //here adjust price incorrect
    if tds.side == "buy".to_string() {
        let price_adj: f64 = adjust_price(tds.price, tick_size, tick_size_keep, true);
        let pdec_bid = Decimal::from_str(&price_adj.to_string()).unwrap();
        let bid_price_f = pdec_bid.round_dp_with_strategy(tick_size_keep, rd);
        let bid_price_f64: f64 = bid_price_f.to_f64().unwrap();

        let mut amount_inpos: f64 = 0.;
        if dinfo.bids.contains_key(&OrderedFloat(bid_price_f64)) {
            amount_inpos = dinfo.bids[&OrderedFloat(bid_price_f64)];
        }
        info!("cid={} sid={} {} bid add tsize={} tick_size_keep={} price={} adust price={} amount={} amount_inpos={}", tds.client_order_id, tds.sid, tds.from_key, tick_size, tick_size_keep, tds.price, bid_price_f64, tds.amount, amount_inpos);

        if bid_price_f64 >= dinfo.ask1 {
            info!("bid price={} > ask1={}, ignore", bid_price_f64, dinfo.ask1);
            return;
        }

        let p: PendingItem = PendingItem {
            create_ts: tds.create_ts * 1000,
            sid: tds.sid,
            client_order_id: tds.client_order_id.to_string(),
            from_key: tds.from_key.to_string(),
            dup_key: tds.dup_key.to_string(),
            price: bid_price_f64,
            amount: tds.amount,
            amount_init: tds.amount,
            inpos: amount_inpos,
            side: "bid".to_string(),
            backlen: 0.,
            tlen: amount_inpos,
            trade_comsume_amount: 0.,
            target_sid: tds.target_sid,
        };
        if ENGIN_CONF.is_spending_open_dump {
            let rd: RecordDumpItem = RecordDumpItem {
                create_ts: tds.create_ts * 1000,
                update_ts: tds.create_ts * 1000,
                client_order_id: p.client_order_id.to_string(),
                symbol: tinfo.symbol.to_string(),
                ttype: "maker".to_string(),
                sid: tds.sid,
                side: "buy".to_string(),
                price: p.price,
                amount_init: tds.amount,
                amount_update: 0.,
                inpos: amount_inpos,
                tlen: amount_inpos,
                from_key: tds.from_key.to_string(),
                status: "open".to_string(),
                bid1: dinfo.bid1,
                ask1: dinfo.ask1,
            };
            write_to_csv(&rd);
        }

        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&tds.sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(bid_price_f64)) {
                Some(v) => {
                    v.push(p);
                }
                None => {
                    let mut v = Vec::new();
                    v.push(p);
                    porders.phash.insert(OrderedFloat(bid_price_f64), v);
                }
            }
        } else {
            panic!("get lock err");
        };
    } else if tds.side == "sell".to_string() {
        let price_adj: f64 = adjust_price(tds.price, tick_size, tick_size_keep, false);
        let pdec_ask = Decimal::from_str(&price_adj.to_string()).unwrap();
        let ask_price_f = pdec_ask.round_dp_with_strategy(tick_size_keep, ru);
        let ask_price_f64: f64 = ask_price_f.to_f64().unwrap();
        let mut amount_inpos: f64 = 0.;
        if dinfo.asks.contains_key(&OrderedFloat(ask_price_f64)) {
            amount_inpos = dinfo.asks[&OrderedFloat(ask_price_f64)];
        }
        info!("cid={} sid={} {} ask add tsize={} tick_size_keep={} price={} adust price={}  amount={} amount_inpos={}", tds.client_order_id, tds.sid, tds.from_key, tick_size, tick_size_keep, tds.price, ask_price_f64, tds.amount, amount_inpos);

        if ask_price_f64 <= dinfo.bid1 {
            info!("bid price={} < bid1={}, ignore", ask_price_f64, dinfo.bid1);
            return;
        }

        let p: PendingItem = PendingItem {
            create_ts: tds.create_ts * 1000,
            sid: tds.sid,
            client_order_id: tds.client_order_id.to_string(),
            from_key: tds.from_key.to_string(),
            dup_key: tds.dup_key.to_string(),
            price: ask_price_f64,
            amount: tds.amount,
            amount_init: tds.amount,
            inpos: amount_inpos,
            tlen: amount_inpos,
            side: "ask".to_string(),
            backlen: 0.,
            trade_comsume_amount: 0.,
            target_sid: tds.target_sid,
        };
        if ENGIN_CONF.is_spending_open_dump {
            let rd: RecordDumpItem = RecordDumpItem {
                create_ts: tds.create_ts * 1000,
                update_ts: tds.create_ts * 1000,
                client_order_id: p.client_order_id.to_string(),
                symbol: tinfo.symbol.to_string(),
                ttype: "maker".to_string(),
                sid: tds.sid,
                side: "sell".to_string(),
                price: p.price,
                amount_init: tds.amount,
                amount_update: 0.,
                inpos: amount_inpos,
                tlen: amount_inpos,
                from_key: tds.from_key.to_string(),
                status: "open".to_string(),
                bid1: dinfo.bid1,
                ask1: dinfo.ask1,
            };
            write_to_csv(&rd);
        }

        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&tds.sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(ask_price_f64)) {
                Some(v) => {
                    v.push(p);
                }
                None => {
                    let mut v = Vec::new();
                    v.push(p);
                    porders.phash.insert(OrderedFloat(ask_price_f64), v);
                }
            }
        } else {
            panic!("get lock err")
        }
        //info!("S_PENDING_PRICEKEY_ASKS={:?}", S_PENDING_PRICEKEY_ASKS[&tds.sid]);
    } else {
        panic!("side={}", tds.side);
    }
    //info!("add pending, sid={} S_PENDING_PRICEKEY_BIDS={:?}, S_PENDING_PRICEKEY_ASKS={:?}", tds.sid, S_PENDING_PRICEKEY_BIDS[&tds.sid], S_PENDING_PRICEKEY_ASKS[&tds.sid]);
}

#[derive(Debug)]
pub struct PendingItem {
    pub create_ts: i64,
    pub sid: i32,
    pub client_order_id: String,
    pub from_key: String,
    pub dup_key: String,
    pub price: f64,
    pub side: String, //bid ask
    pub amount: f64,
    pub amount_init: f64,
    pub inpos: f64,
    pub backlen: f64,
    pub tlen: f64,
    pub trade_comsume_amount: f64,
    pub target_sid: i32,
}

#[derive(Debug)]
pub struct PendingOrdersInPos {
    pub phash: HashMap<OrderedFloat<f64>, Vec<PendingItem>>,
}
