#[path = "../gconf_stream.rs"]
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
use std::path::Path;

use crate::gconf::ENGIN_CONF;
use crate::ordertake::add_taking;
use crate::record::{init_record_pub_from_market_ipc, write_to_csv, RecordDumpItem};
use crate::spending::{
    add_pending, clear_pending, drop_pending, S_PENDING_PRICEKEY_ASKS, S_PENDING_PRICEKEY_BIDS,
};
use crate::symbolinfo::get_market;
use crate::trade::{DepthInfo, TradeInfo};
use zmq::Message as ZmqMessage;

#[derive(Debug, Deserialize)]
#[serde(tag = "event")]
enum StreamEvent {
    #[serde(rename = "tick")]
    Tick { symbol: String, ts_s: i64 },
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

struct Args {
    ipc: Option<String>,
}

fn parse_args() -> Args {
    let mut ipc = None;
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ipc" => ipc = iter.next(),
            _ => {}
        }
    }
    Args { ipc }
}

fn symbol_from_ipc_path(path: &str) -> Option<String> {
    let file = Path::new(path).file_name()?.to_string_lossy();
    let stem = Path::new(file.as_ref()).file_stem()?.to_string_lossy();
    let symbol = stem.trim();
    if symbol.is_empty() {
        None
    } else {
        Some(symbol.to_string())
    }
}

fn normalize_ipc_endpoint(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let trimmed = trimmed.trim_end_matches('/');
    if trimmed.starts_with("ipc://") {
        Some(trimmed.to_string())
    } else {
        Some(format!("ipc://{}", trimmed))
    }
}

struct BinaryEvent {
    event: u8,
    side_id: u8,
    is_snapshot: u8,
    origin: u8,
    ts_us: i64,
    price: f64,
    amount: f64,
}

fn decode_binary_event(msg: &ZmqMessage) -> Option<BinaryEvent> {
    if msg.len() != 28 {
        warn!("invalid ipc message length: {}", msg.len());
        return None;
    }
    let buf = msg.as_ref();
    Some(BinaryEvent {
        event: buf[0],
        side_id: buf[1],
        is_snapshot: buf[2],
        origin: buf[3],
        ts_us: i64::from_le_bytes(buf[4..12].try_into().ok()?),
        price: f64::from_le_bytes(buf[12..20].try_into().ok()?),
        amount: f64::from_le_bytes(buf[20..28].try_into().ok()?),
    })
}

fn origin_to_sid(origin: u8) -> Option<i32> {
    if ENGIN_CONF.vsids.len() < 2 {
        warn!("vsids length < 2, cannot map origin");
        return None;
    }
    match origin {
        0 => Some(ENGIN_CONF.vsids[0]),
        1 => Some(ENGIN_CONF.vsids[1]),
        _ => {
            warn!("invalid origin {}, expected 0/1", origin);
            None
        }
    }
}

fn record_pendings(tsms: i64, tinfo: &mut TradeInfo) {
    for sid in &ENGIN_CONF.vsids {
        if let Some(porders) =
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

                    if ENGIN_CONF.is_spending_tick_dump && (tsms % ENGIN_CONF.tick_dump_modts) == 0
                    {
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

        if let Some(porders) =
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

                    if ENGIN_CONF.is_spending_tick_dump && (tsms % ENGIN_CONF.tick_dump_modts) == 0
                    {
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
    let v = vec![
        ts_us as f64,
        0.0,
        side_id as f64,
        price,
        amount,
        0.0,
        sid as f64,
    ];
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

fn handle_binary_event(tinfo: &mut TradeInfo, event: BinaryEvent) {
    match event.event {
        1 => {
            let sid = match origin_to_sid(event.origin) {
                Some(v) => v,
                None => return,
            };
            if !ENGIN_CONF.vsids.contains(&sid) {
                warn!("ignore inc sid={}, not in vsids", sid);
                return;
            }
            let v = vec![
                event.ts_us as f64,
                event.is_snapshot as f64,
                event.side_id as f64,
                event.price,
                event.amount,
                1.0,
                sid as f64,
            ];
            lprocess::process(&v, tinfo);
        }
        2 => {
            let sid = match origin_to_sid(event.origin) {
                Some(v) => v,
                None => return,
            };
            if !ENGIN_CONF.vsids.contains(&sid) {
                warn!("ignore trade sid={}, not in vsids", sid);
                return;
            }
            let v = vec![
                event.ts_us as f64,
                0.0,
                event.side_id as f64,
                event.price,
                event.amount,
                0.0,
                sid as f64,
            ];
            tprocess::process(&v, tinfo);
        }
        3 => {
            if ENGIN_CONF.vsids.len() < 2 {
                warn!("tick ignored: vsids length < 2");
                return;
            }
            let ts_s = event.ts_us as f64 / 1_000_000.0;
            let v = vec![ts_s];
            do_tick(&v, tinfo);
        }
        _ => {
            warn!("invalid event type {}", event.event);
        }
    }
}

fn run_ipc(ipc_path: &str, symbol: &str) {
    let ctx = zmq::Context::new();
    let socket = match ctx.socket(zmq::SUB) {
        Ok(v) => v,
        Err(err) => {
            warn!("zmq create socket failed: {}", err);
            return;
        }
    };
    if let Err(err) = socket.set_subscribe(b"") {
        warn!("zmq subscribe failed: {}", err);
        return;
    }
    let endpoint = match normalize_ipc_endpoint(ipc_path) {
        Some(v) => v,
        None => {
            warn!("invalid ipc path: {}", ipc_path);
            return;
        }
    };
    if let Err(err) = socket.connect(&endpoint) {
        warn!("zmq connect failed: {}", err);
        return;
    }

    let mut state = StreamState::new();
    loop {
        let msg = match socket.recv_msg(0) {
            Ok(v) => v,
            Err(err) => {
                warn!("zmq recv failed: {}", err);
                break;
            }
        };
        let Some(event) = decode_binary_event(&msg) else {
            continue;
        };
        let tinfo = match state.get_tinfo(symbol) {
            Some(v) => v,
            None => continue,
        };
        handle_binary_event(tinfo, event);
    }
}

fn run_stdin() {
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

fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    if ENGIN_CONF.stg != "pairmm_two_exchange_simple"
        && ENGIN_CONF.stg != "pairmm_one_exchange_simple"
    {
        warn!(
            "stream_pairmm is intended for pairmm_two_exchange_simple/pairmm_one_exchange_simple, stg={}",
            ENGIN_CONF.stg
        );
    }

    if ENGIN_CONF.stg == "pairmm_one_exchange_simple" {
        stgs::pairmm_one_exchange_simple::clear_ongoing_pending();
    } else if ENGIN_CONF.stg == "pairmm_two_exchange_simple" {
        stgs::pairmm_two_exchange_simple::clear_ongoing_pending();
    }
    clear_pending();

    let args = parse_args();
    if let Some(ipc_path) = args.ipc {
        init_record_pub_from_market_ipc(&ipc_path);
        let symbol = symbol_from_ipc_path(&ipc_path).unwrap_or_default();
        if symbol.is_empty() {
            warn!("ipc mode requires ipc path with symbol filename");
            return;
        }
        run_ipc(&ipc_path, &symbol);
    } else {
        run_stdin();
    }
}
