use crate::gconf::ENGIN_CONF;
use crate::record::{write_to_csv, write_to_csv_ts, RecordDumpItem, TSRecordItem};
use crate::spending::{PendingItem, S_PENDING_PRICEKEY_ASKS, S_PENDING_PRICEKEY_BIDS};
use crate::stg::cb_filled;
use crate::stg::cb_finished;
use crate::symbolinfo::get_market;
use crate::target_sid_open::add;
use crate::trade::{DepthInfo, TradeInfo};
use log::info;
use ordered_float::OrderedFloat;
use std::time::Duration;

const COL_TSUS: usize = 0;
//const COL_IS: usize   = 1;
const COL_SIDEID: usize = 2;
const COL_PRICE: usize = 3;
const COL_AMOUNT: usize = 4;
// const COL_TID: usize = 5;
const COL_SID: usize = 6;

pub fn deal_order(
    ts: i64,
    sid: &i32,
    tinfo: &mut TradeInfo,
    pitem: &PendingItem,
    side: &str,
    client_order_id: &str,
    _price: f64,
    amount: f64,
    amount_traded: f64,
    make_amout: f64,
    is_full_filled: bool,
) {
    info!("deal order, side={} client_order_id={} amount_traded={} amount={} make_amout={} is_full_filled={}", side, client_order_id, amount_traded, amount, make_amout, is_full_filled);
    let e = &ENGIN_CONF.sids[sid];
    let market = get_market()
        .get((e["exchange"].to_string() + ":" + &e["etype"] + ":" + &tinfo.symbol_std).as_str())
        .unwrap();
    let dinfo: &mut DepthInfo = tinfo.depths.get_mut(sid).unwrap();

    let mut contract_value = 1.;
    if e["etype"] == "swap" && e["exchange"] == "okx" {
        contract_value = market.contract_value.unwrap();
        println!("contract_value={}", contract_value);
    }
    if side == "buy" {
        tinfo.open += make_amout * contract_value;
        let p = tinfo.pos.get_mut(sid).unwrap();
        info!("deal order buy u={}", make_amout * contract_value * _price);
        *p += make_amout;
    } else if side == "sell" {
        tinfo.open -= make_amout * contract_value;
        let p = tinfo.pos.get_mut(sid).unwrap();
        info!("deal order sell u={}", make_amout * contract_value * _price);
        *p -= make_amout;
    }
    let mut status: String = "partial_filled".to_string();
    if is_full_filled {
        status = "filled".to_string();
    }
    let update_ts_ms = (ts as f64 / 1000.) as i64;
    let rd: RecordDumpItem = RecordDumpItem {
        create_ts: pitem.create_ts,
        update_ts: update_ts_ms,
        client_order_id: pitem.client_order_id.to_string(),
        symbol: tinfo.symbol.to_string(),
        ttype: "maker".to_string(),
        sid: *sid,
        side: side.to_string(),
        price: pitem.price,
        amount_init: pitem.amount_init * contract_value,
        amount_update: make_amout * contract_value,
        status: status,
        inpos: pitem.inpos,
        tlen: pitem.tlen,
        from_key: pitem.from_key.to_string(),
        bid1: dinfo.bid1,
        ask1: dinfo.ask1,
    };
    write_to_csv(&rd);
    let p = tinfo.pos.get(sid).unwrap();
    let tsrd: TSRecordItem = TSRecordItem {
        create_ts: update_ts_ms,
        symbol: tinfo.symbol.to_string(),
        sid: *sid,
        pos: *p,
        open: tinfo.open,
    };
    write_to_csv_ts(&tsrd);

    cb_filled(tinfo, pitem, make_amout * contract_value);

    if ENGIN_CONF.is_target_sid_open_keep {
        if side == "buy" {
            add(&pitem.target_sid, make_amout * contract_value);
        } else if side == "sell" {
            add(&pitem.target_sid, -make_amout * contract_value);
        }
    }
}

pub fn process(t: &Vec<f64>, tinfo: &mut TradeInfo) {
    let price_trade: f64 = t[COL_PRICE];
    let amount_trade: f64 = t[COL_AMOUNT];
    let sid: i32 = t[COL_SID] as i32;
    let exists = ENGIN_CONF.vsids.iter().any(|&x| x == sid);
    if !exists {
        return;
    }

    let ts = t[COL_TSUS] as i64;

    //info!("trade side={} sid={} amount={} price={}",t[COL_SIDEID], t[COL_SID], amount_trade, price_trade);
    if t[COL_SIDEID] == 0. {
        //buy, check ask queues
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1))
        {
            let mut vdel: Vec<OrderedFloat<f64>> = Vec::new();
            for (price, vpitem) in porders.phash.iter_mut() {
                if price_trade >= price.into_inner() {
                    let mut cid: usize = 0;
                    let mut vitems: Vec<usize> = Vec::new();
                    for pitem in vpitem.iter_mut() {
                        //info!("bid coid={} amount={} price={}", &pitem.client_order_id, amount_trade, price_trade);
                        let pos = pitem.inpos - pitem.trade_comsume_amount;
                        let mut taker_amount = amount_trade - pos;
                        //taker_amount = taker_amount.min(pitem.amount);
                        if taker_amount <= 0. {
                            pitem.trade_comsume_amount += amount_trade;
                        } else if taker_amount <= pitem.amount {
                            pitem.amount -= taker_amount;
                            pitem.trade_comsume_amount =
                                pitem.trade_comsume_amount + amount_trade - taker_amount;
                            //部分成交
                            deal_order(
                                ts,
                                &sid,
                                tinfo,
                                pitem,
                                "sell",
                                &pitem.client_order_id,
                                pitem.price,
                                pitem.amount,
                                pitem.amount_init - pitem.amount,
                                taker_amount,
                                false,
                            );
                        } else if taker_amount > pitem.amount {
                            //deal_order(&exchange, &etype , &symbol_std, &pitem.account_id, &pitem.client_order_id, pitem.amount_init,pitem.amount, "sell",true);

                            deal_order(
                                ts,
                                &sid,
                                tinfo,
                                pitem,
                                "sell",
                                &pitem.client_order_id,
                                pitem.price,
                                pitem.amount,
                                pitem.amount_init,
                                pitem.amount,
                                true,
                            );
                            pitem.amount = 0.;
                            cb_finished((t[0] as i64) / 1000, tinfo, pitem, false);
                            // println!("process=========={}",(t[0] as i64)/1000);
                            vitems.push(cid);
                        } else {
                            panic!(
                                "taker amount[{}] > amount_left[{}]",
                                taker_amount, pitem.amount
                            );
                        }
                        cid += 1;
                    }
                    let mut rmove_count: usize = 0;
                    for v in vitems {
                        vpitem.remove(v - rmove_count);
                        rmove_count += 1;
                    }
                    if vpitem.len() == 0 {
                        vdel.push(price.clone());
                    }
                }
            }
            for p in vdel {
                porders.phash.remove(&p);
            }
        } else {
            panic!("cannot aquire lock");
        }
    } else if t[COL_SIDEID] == 1. {
        //sell, check bid queues
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(Duration::from_secs(1))
        {
            let mut vdel: Vec<OrderedFloat<f64>> = Vec::new();
            for (price, vpitem) in porders.phash.iter_mut() {
                if price_trade <= price.into_inner() {
                    let mut cid: usize = 0;

                    let mut vitems: Vec<usize> = Vec::new();
                    for pitem in vpitem.iter_mut() {
                        info!(
                            "ask coid={} amount={} price={}",
                            &pitem.client_order_id, amount_trade, price_trade
                        );

                        let pos = pitem.inpos - pitem.trade_comsume_amount;
                        let mut taker_amount = amount_trade - pos;
                        //taker_amount = taker_amount.min(pitem.amount);

                        if taker_amount <= 0. {
                            pitem.trade_comsume_amount += amount_trade;
                        } else if taker_amount <= pitem.amount {
                            pitem.amount -= taker_amount;

                            pitem.trade_comsume_amount =
                                pitem.trade_comsume_amount + amount_trade - taker_amount;
                            deal_order(
                                ts,
                                &sid,
                                tinfo,
                                pitem,
                                "buy",
                                &pitem.client_order_id,
                                pitem.price,
                                pitem.amount,
                                pitem.amount_init - pitem.amount,
                                taker_amount,
                                false,
                            );
                        } else if taker_amount > pitem.amount {
                            deal_order(
                                ts,
                                &sid,
                                tinfo,
                                pitem,
                                "buy",
                                &pitem.client_order_id,
                                pitem.price,
                                pitem.amount,
                                pitem.amount_init,
                                pitem.amount,
                                true,
                            );
                            info!("oid={} order finish", pitem.client_order_id);
                            pitem.amount = 0.;
                            cb_finished((t[0] as i64) / 1000, tinfo, pitem, false);
                            // println!("process=========={}",(t[0] as i64)/1000);
                            vitems.push(cid);
                        } else {
                            panic!(
                                "taker amount[{}] > amount_left[{}]",
                                taker_amount, pitem.amount
                            );
                        }

                        cid += 1;
                    }
                    let mut rmove_count: usize = 0;
                    for v in vitems {
                        // println!("Sell===========vpitem-len==={} cid =={}", vpitem.len(), v);

                        vpitem.remove(v - rmove_count);
                        rmove_count += 1;
                    }

                    if vpitem.len() == 0 {
                        vdel.push(price.clone());
                    }
                }
            }
            for p in vdel {
                porders.phash.remove(&p);
            }
        } else {
            panic!("cannot aquire lock");
        }
    } else {
        panic!("side={}", t[COL_SIDEID]);
    }
}
