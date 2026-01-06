#[path = "../gconf.rs"]
mod gconf;
#[path = "../lprocess.rs"]
mod lprocess;
#[path = "../ordertake.rs"]
mod ordertake;
#[path = "../record.rs"]
mod record;
#[path = "../spending.rs"]
mod spending;
#[path = "../stg.rs"]
mod stg;
#[path = "../stgs/mod.rs"]
mod stgs;
#[path = "../symbolinfo.rs"]
mod symbolinfo;
#[path = "../target_sid_open.rs"]
mod target_sid_open;
#[path = "../tprocess.rs"]
mod tprocess;
#[path = "../trade.rs"]
mod trade;

use log::{info, warn};
use ordered_float::OrderedFloat;
use serde::Deserialize;
use std::io::{self, BufRead};

use crate::gconf::ENGIN_CONF;
use crate::record::{write_to_csv, RecordDumpItem};
use crate::ordertake::add_taking;
use crate::spending::{
    add_pending, clear_pending, drop_pending, S_PENDING_PRICEKEY_ASKS, S_PENDING_PRICEKEY_BIDS,
};
use crate::symbolinfo::get_market;
use crate::trade::{DepthInfo, TradeInfo};

#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
enum StreamEvent {
    #[serde(rename = "tick")]
    Tick {
        symbol: String,
        ts_s: i64,
    },
    #[serde(rename = "inc")]
    Inc {
        symbol: String,
        ts_us: i64,
        sid: i32,
        is_snapshot: i32,
        side_id: i32,
        price: f64,
        amount: f64,
    },
    #[serde(rename = "trade")]
    Trade {
        symbol: String,
        ts_us: i64,
        sid: i32,
        side_id: i32,
        price: f64,
        amount: f64,
    },
}

struct StreamState {
    symbol: Option<String>,
    tinfo: Option<TradeInfo>,
}

impl StreamState {
    fn new() -> Self {
        Self {
            symbol: None,
            tinfo: None,
        }
    }

    fn get_tinfo<'a>(&'a mut self, symbol: &str) -> Option<&'a mut TradeInfo> {
        match &self.symbol {
            None => {
                let tinfo = TradeInfo::new(symbol.to_string(), &ENGIN_CONF.sids);
                stg::cb_init(&tinfo);
                self.symbol = Some(symbol.to_string());
                self.tinfo = Some(tinfo);
            }
            Some(curr) => {
                if curr != symbol {
                    warn!(
                        "stream_pairmm single-symbol mode, ignore symbol={} current={}",
                        symbol, curr
                    );
                    return None;
                }
            }
        }
        self.tinfo.as_mut()
    }
}

fn record_pendings(tsms: i64, tinfo: &mut TradeInfo) {
    for sid in &ENGIN_CONF.vsids {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(std::time::Duration::from_secs(1))
        {
            let mut _vdel: Vec<OrderedFloat<f64>> = Vec::new();
            for (_price, vpitem) in porders.phash.iter() {
                for pitem in vpitem {
                    let e = &ENGIN_CONF.sids[sid];
                    let market = get_market()
                        .get(
                            (e["exchange"].to_string()
                                + ":"
                                + &e["etype"]
                                + ":"
                                + &tinfo.symbol_std)
                                .as_str(),
                        )
                        .unwrap();

                    let mut contract_value = 1.0;
                    if e["etype"] == "swap" && e["exchange"] == "okx" {
                        contract_value = market.contract_value.unwrap();
                    }

                    if ENGIN_CONF.is_spending_tick_dump && (tsms % ENGIN_CONF.tick_dump_modts) == 0 {
                        let dinfo: &mut DepthInfo = tinfo.depths.get_mut(sid).unwrap();
                        let update_ts_ms = tsms;
                        let rd: RecordDumpItem = RecordDumpItem {
                            create_ts: pitem.create_ts,
                            update_ts: update_ts_ms,
                            client_order_id: pitem.client_order_id.to_string(),
                            symbol: tinfo.symbol.to_string(),
                            ttype: "maker".to_string(),
                            sid: *sid,
                            side: pitem.side.to_string(),
                            price: pitem.price,
                            amount_init: pitem.amount_init * contract_value,
                            amount_update: 0.0,
                            tlen: pitem.tlen,
                            inpos: pitem.inpos,
                            status: "tickupdate".to_string(),
                            from_key: pitem.from_key.to_string(),
                            bid1: dinfo.bid1,
                            ask1: dinfo.ask1,
                        };
                        write_to_csv(&rd);
                    }
                }
            }
        }

        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(std::time::Duration::from_secs(1))
        {
            let mut _vdel: Vec<OrderedFloat<f64>> = Vec::new();
            for (_price, vpitem) in porders.phash.iter() {
                for pitem in vpitem {
                    let e = &ENGIN_CONF.sids[sid];
                    let market = get_market()
                        .get(
                            (e["exchange"].to_string()
                                + ":"
                                + &e["etype"]
                                + ":"
                                + &tinfo.symbol_std)
                                .as_str(),
                        )
                        .unwrap();

                    let mut contract_value = 1.0;
                    if e["etype"] == "swap" && e["exchange"] == "okx" {
                        contract_value = market.contract_value.unwrap();
                    }

                    if ENGIN_CONF.is_spending_tick_dump && (tsms % ENGIN_CONF.tick_dump_modts) == 0 {
                        let dinfo: &mut DepthInfo = tinfo.depths.get_mut(sid).unwrap();
                        let update_ts_ms = tsms;
                        let rd: RecordDumpItem = RecordDumpItem {
                            create_ts: pitem.create_ts,
                            update_ts: update_ts_ms,
                            client_order_id: pitem.client_order_id.to_string(),
                            symbol: tinfo.symbol.to_string(),
                            ttype: "maker".to_string(),
                            sid: *sid,
                            side: pitem.side.to_string(),
                            price: pitem.price,
                            amount_init: pitem.amount_init * contract_value,
                            amount_update: 0.0,
                            tlen: pitem.tlen,
                            inpos: pitem.inpos,
                            status: "tickupdate".to_string(),
                            from_key: pitem.from_key.to_string(),
                            bid1: dinfo.bid1,
                            ask1: dinfo.ask1,
                        };
                        write_to_csv(&rd);
                    }
                }
            }
        }
    }
}

fn do_tick(v: &Vec<f64>, tinfo: &mut TradeInfo) {
    let s = stg::tick(v, tinfo);
    let tds = s.0;
    let cds = s.1;
    let tsms = (v[0] * 1000.0) as i64;

    for td in tds {
        info!("td={:?}", td);
        if td.ttype == "maker".to_string() {
            add_pending(tinfo, &td);
        } else if td.ttype == "taker".to_string() {
            add_taking(tinfo, &td);
        }
    }

    for cd in cds {
        drop_pending((v[0] * 1000.0) as i64, tinfo, &cd);
    }

    record_pendings(tsms, tinfo)
}

fn handle_inc(tinfo: &mut TradeInfo, row: &StreamEvent) {
    let (ts_us, sid, is_snapshot, side_id, price, amount) = match row {
        StreamEvent::Inc {
            ts_us,
            sid,
            is_snapshot,
            side_id,
            price,
            amount,
            ..
        } => (*ts_us, *sid, *is_snapshot, *side_id, *price, *amount),
        _ => return,
    };
    let v = vec![
        ts_us as f64,
        is_snapshot as f64,
        side_id as f64,
        price,
        amount,
        1.0,
        sid as f64,
    ];
    lprocess::process(&v, tinfo);
}

fn handle_trade(tinfo: &mut TradeInfo, row: &StreamEvent) {
    let (ts_us, sid, side_id, price, amount) = match row {
        StreamEvent::Trade {
            ts_us,
            sid,
            side_id,
            price,
            amount,
            ..
        } => (*ts_us, *sid, *side_id, *price, *amount),
        _ => return,
    };
    let v = vec![ts_us as f64, 0.0, side_id as f64, price, amount, 0.0, sid as f64];
    tprocess::process(&v, tinfo);
}

fn handle_tick(tinfo: &mut TradeInfo, row: &StreamEvent) {
    let ts_s = match row {
        StreamEvent::Tick { ts_s, .. } => *ts_s,
        _ => return,
    };
    let v = vec![ts_s as f64];
    do_tick(&v, tinfo);
}

fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    if ENGIN_CONF.stg != "pairmm_simple" {
        warn!(
            "stream_pairmm is intended for pairmm_simple, stg={}",
            ENGIN_CONF.stg
        );
    }

    stgs::pairmm_simple::clear_ongoing_pending();
    clear_pending();

    let stdin = io::stdin();
    let mut state = StreamState::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(v) => v,
            Err(err) => {
                warn!("read line failed: {}", err);
                break;
            }
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let event: StreamEvent = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(err) => {
                warn!("invalid json line: {} err={}", line, err);
                continue;
            }
        };

        let symbol = match &event {
            StreamEvent::Tick { symbol, .. } => symbol,
            StreamEvent::Inc { symbol, .. } => symbol,
            StreamEvent::Trade { symbol, .. } => symbol,
        };

        let tinfo = match state.get_tinfo(symbol) {
            Some(v) => v,
            None => continue,
        };

        match event {
            StreamEvent::Tick { .. } => handle_tick(tinfo, &event),
            StreamEvent::Inc { sid, .. } => {
                if !ENGIN_CONF.vsids.contains(&sid) {
                    warn!("ignore inc sid={}, not in vsids", sid);
                    continue;
                }
                handle_inc(tinfo, &event);
            }
            StreamEvent::Trade { sid, .. } => {
                if !ENGIN_CONF.vsids.contains(&sid) {
                    warn!("ignore trade sid={}, not in vsids", sid);
                    continue;
                }
                handle_trade(tinfo, &event);
            }
        }
    }
}
