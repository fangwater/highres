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
use log4rs;
use ordered_float::OrderedFloat;

use crate::gconf::ENGIN_CONF;
use crate::record::{write_to_csv, RecordDumpItem};
use crate::spending::{
    add_pending, add_taking, clear_pending, drop_pending, S_PENDING_PRICEKEY_ASKS,
    S_PENDING_PRICEKEY_BIDS,
};
use crate::symbolinfo::get_market;
use crate::trade::{DepthInfo, TradeInfo};

#[derive(Debug)]
enum MockEvent {
    Tick(Vec<f64>),
    Mm(Vec<f64>),
}

#[derive(Debug)]
struct TimedEvent {
    ts_us: i64,
    event: MockEvent,
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

fn do_mm(v: &Vec<f64>, tinfo: &mut TradeInfo) {
    if v[5] == 1.0 {
        lprocess::process(v, tinfo);
    } else if v[5] == 0.0 {
        tprocess::process(v, tinfo);
    }
}

fn tick_row(
    ts_s: i64,
    signal1: i64,
    signal2: i64,
    signal3: i64,
    buy_cancel: i64,
    sell_cancel: i64,
) -> Vec<f64> {
    vec![
        ts_s as f64,
        signal1 as f64,
        signal2 as f64,
        signal3 as f64,
        buy_cancel as f64,
        sell_cancel as f64,
    ]
}

fn inc_row(ts_us: i64, sid: i32, is_snapshot: i32, side_id: i32, price: f64, amount: f64) -> Vec<f64> {
    vec![
        ts_us as f64,
        is_snapshot as f64,
        side_id as f64,
        price,
        amount,
        1.0,
        sid as f64,
    ]
}

fn trade_row(ts_us: i64, sid: i32, side_id: i32, price: f64, amount: f64) -> Vec<f64> {
    vec![ts_us as f64, 0.0, side_id as f64, price, amount, 0.0, sid as f64]
}

fn add_snapshot(rows: &mut Vec<Vec<f64>>, base_ts_us: i64, sid: i32, bid1: f64, ask1: f64) {
    rows.push(inc_row(base_ts_us, sid, 1, 0, bid1, 5.0));
    rows.push(inc_row(base_ts_us, sid, 1, 0, bid1 - 0.5, 3.0));
    rows.push(inc_row(base_ts_us, sid, 1, 1, ask1, 5.0));
    rows.push(inc_row(base_ts_us, sid, 1, 1, ask1 + 0.5, 3.0));
    rows.push(inc_row(base_ts_us + 100, sid, 0, 0, bid1, 5.0));
    rows.push(inc_row(base_ts_us + 100, sid, 0, 1, ask1, 5.0));
}

fn build_mock_events(sid_a: i32, sid_b: i32) -> Vec<TimedEvent> {
    let mut events = Vec::new();
    let mut mm_rows = Vec::new();

    add_snapshot(&mut mm_rows, 1_000_000, sid_a, 100.0, 100.5);
    add_snapshot(&mut mm_rows, 1_000_000, sid_b, 100.1, 100.6);

    mm_rows.push(trade_row(10_000_500, sid_a, 1, 99.9, 2.0));
    mm_rows.push(trade_row(20_000_500, sid_a, 0, 100.8, 1.5));
    mm_rows.push(trade_row(20_000_800, sid_b, 1, 100.0, 1.0));

    for row in mm_rows {
        events.push(TimedEvent {
            ts_us: row[0] as i64,
            event: MockEvent::Mm(row),
        });
    }

    let tick_rows = vec![
        tick_row(10, 1, 1, 1, 0, 0),
        tick_row(20, -1, 1, 1, 0, 0),
        tick_row(30, 0, 0, 0, 1, 1),
    ];
    for row in tick_rows {
        events.push(TimedEvent {
            ts_us: (row[0] * 1_000_000.0) as i64,
            event: MockEvent::Tick(row),
        });
    }

    events.sort_by_key(|e| e.ts_us);
    events
}

fn run_mock(symbol: &str, sid_a: i32, sid_b: i32) {
    info!("mock v6 start symbol={}", symbol);
    let mut tinfo = TradeInfo::new(symbol.to_string(), &ENGIN_CONF.sids);
    stg::cb_init(&tinfo);

    let events = build_mock_events(sid_a, sid_b);
    for timed in events {
        match timed.event {
            MockEvent::Tick(row) => do_tick(&row, &mut tinfo),
            MockEvent::Mm(row) => do_mm(&row, &mut tinfo),
        }
    }
}

fn main() {
    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();

    if ENGIN_CONF.stg != "sampling_v6" {
        warn!("mock_v6 is intended for sampling_v6, stg={}", ENGIN_CONF.stg);
    }

    if ENGIN_CONF.symbols.is_empty() {
        warn!("no symbols configured, exit");
        return;
    }

    if ENGIN_CONF.vsids.len() < 2 {
        warn!("mock_v6 expects at least 2 sids, vsids={:?}", ENGIN_CONF.vsids);
    }

    let sid_a = *ENGIN_CONF.vsids.get(0).unwrap_or(&0);
    let sid_b = *ENGIN_CONF.vsids.get(1).unwrap_or(&sid_a);

    for symbol in &ENGIN_CONF.symbols {
        stgs::sampling_v6::clear_ongoing_pending();
        run_mock(symbol, sid_a, sid_b);
        clear_pending();
    }
}
