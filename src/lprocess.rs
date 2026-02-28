use crate::spending::{S_PENDING_PRICEKEY_ASKS, S_PENDING_PRICEKEY_BIDS};
use crate::trade::{DepthInfo, TradeInfo};
use log::debug;
use ordered_float::OrderedFloat;
use std::time::Duration;

//const COL_TSUS: usize = 0;
const COL_IS: usize = 1;
const COL_SIDEID: usize = 2;
const COL_PRICE: usize = 3;
const COL_AMOUNT: usize = 4;
// const COL_TID: usize = 5;
const COL_SID: usize = 6;
use crate::gconf::{ENGIN_CONF, PAIRMM_CONF};

pub fn pending_adjust(
    sid: i32,
    side: &str,
    price: f64,
    mut delta_amount: f64,
    amount_in_price: f64,
) {
    if side == "bid" {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(price)) {
                Some(vs) => {
                    for v in vs {
                        //info!("bid pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={} trade_comsume_amount={} bl={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price, v.trade_comsume_amount, v.backlen);
                        v.tlen = amount_in_price;
                        if v.trade_comsume_amount > 0. {
                            if v.inpos - v.trade_comsume_amount < 0. {
                                v.inpos = 0.;
                                delta_amount += v.inpos - v.trade_comsume_amount;
                            } else {
                                v.inpos -= v.trade_comsume_amount;
                                delta_amount += v.trade_comsume_amount;
                            }

                            v.trade_comsume_amount = 0.;
                        }

                        if delta_amount > 0. {
                            v.backlen += delta_amount;
                        } else if delta_amount < 0. {
                            if v.inpos + v.backlen > 0. {
                                v.inpos =
                                    v.inpos - (-delta_amount * (v.inpos / (v.inpos + v.backlen)));
                                if v.inpos < 0. {
                                    v.inpos = 0.
                                }
                            } else {
                                v.inpos = 0.
                            }

                            if v.inpos + v.backlen > 0. {
                                v.backlen = v.backlen
                                    - (-delta_amount * (v.backlen / (v.inpos + v.backlen)));
                                if v.backlen < 0. {
                                    v.backlen = 0.
                                }
                            } else {
                                v.backlen = 0.
                            }
                        }
                        //info!("finish bid pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price);
                    }
                }
                None => {}
            }
        } else {
            panic!("lock-err===== ");
        }
    } else if side == "ask" {
        if let Some(mut porders) =
            S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1))
        {
            match porders.phash.get_mut(&OrderedFloat(price)) {
                Some(vs) => {
                    for v in vs {
                        //info!("ask pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={} trade_comsume_amount={} bl={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price,v.trade_comsume_amount, v.backlen);
                        v.tlen = amount_in_price;

                        if v.trade_comsume_amount > 0. {
                            if v.inpos - v.trade_comsume_amount < 0. {
                                v.inpos = 0.;
                                delta_amount += v.inpos - v.trade_comsume_amount;
                            } else {
                                v.inpos -= v.trade_comsume_amount;
                                delta_amount += v.trade_comsume_amount;
                            }
                        }

                        if delta_amount > 0. {
                            v.backlen += delta_amount;
                        } else if delta_amount < 0. {
                            if v.inpos + v.backlen > 0. {
                                v.inpos =
                                    v.inpos - (-delta_amount * (v.inpos / (v.inpos + v.backlen)));
                                if v.inpos < 0. {
                                    v.inpos = 0.
                                }
                            } else {
                                v.inpos = 0.
                            }

                            if v.inpos + v.backlen > 0. {
                                v.backlen = v.backlen
                                    - (-delta_amount * (v.backlen / (v.inpos + v.backlen)));
                                if v.backlen < 0. {
                                    v.backlen = 0.
                                }
                            } else {
                                v.backlen = 0.
                            }
                        }
                        //info!("finish ask pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price);
                    }
                }
                None => {}
            }
        } else {
            panic!("lock-err===== ");
        }
    } else {
        panic!("side={}", side);
    }
}

pub fn correct(dinfo: &mut DepthInfo, sid: i32, side: &str, price: f64) {
    let mut delist = Vec::new();

    if side == "bid" {
        for (dprice, damount) in dinfo.asks.iter() {
            debug!(
                "dupdate sid={} bid price={}, test ask[{}]",
                sid, price, dprice
            );
            if price >= dprice.into_inner() {
                delist.push(dprice.into_inner());
                debug!("wrong bid position, drop");
            } else {
                break;
            }
        }
        for d in delist.iter() {
            debug!("drop ask price={}", *d);
            dinfo.asks.remove(&OrderedFloat(*d));
        }
    } else if side == "ask" {
        for (dprice, damount) in dinfo.bids.iter().rev() {
            debug!(
                "dupdate sid={} ask price={}, test bid[{}]",
                sid, price, dprice
            );
            if price <= dprice.into_inner() {
                debug!("wrong bid position, drop");
                delist.push(dprice.into_inner());
            } else {
                break;
            }
        }
        for d in delist.iter() {
            debug!("drop bid price={}", *d);
            dinfo.bids.remove(&OrderedFloat(*d));
        }
    }
}

pub fn process(v: &Vec<f64>, tinfo: &mut TradeInfo) {
    let sid = v[COL_SID] as i32;

    let exists = ENGIN_CONF.vsids.iter().any(|&x| x == sid);
    if !exists {
        return;
    }

    let dinfo: &mut DepthInfo = tinfo.depths.get_mut(&sid).unwrap();
    let price = v[COL_PRICE];
    let amount = v[COL_AMOUNT];
    let ts_s = (v[0] / 1_000_000.0) as i64;

    if v[COL_IS] != 1. && dinfo.first_inc_ts_s == 0 {
        dinfo.first_inc_ts_s = ts_s;
    }

    if v[COL_IS] == 1. {
        //build snap
        if !dinfo.is_snaping {
            dinfo.bids.clear();
            dinfo.asks.clear();
        }
        dinfo.is_snaping = true;
        dinfo.is_finish_snap = false;
        if v[COL_SIDEID] == 0. {
            debug!("insert (is) bid={} amt={}", v[COL_PRICE], v[COL_AMOUNT]);
            dinfo.bids.insert(OrderedFloat(price), amount);
        }
        if v[COL_SIDEID] == 1. {
            debug!("insert (is) ask={} amt={}", v[COL_PRICE], v[COL_AMOUNT]);
            dinfo.asks.insert(OrderedFloat(price), amount);
        }
    } else {
        if dinfo.is_snaping == true {
            dinfo.is_finish_snap = true;
            dinfo.is_snaping = false;
        }

        // info!("up sid={} price={} [{} {}] is_finish_snap={} is_snaping={}", sid, price, dinfo.bid1, dinfo.ask1, dinfo.is_finish_snap, dinfo.is_snaping);
        // info!("sid={} bids={:?}", sid, dinfo.bids);
        // info!("sid={} asks={:?}", sid, dinfo.asks);
        if v[COL_SIDEID] == 0. {
            //bid update
            let delta_amount: f64;

            match dinfo.bids.get(&OrderedFloat(price)) {
                Some(amount_in_price) => {
                    delta_amount = amount - amount_in_price;
                }
                None => {
                    delta_amount = amount;
                }
            }

            pending_adjust(sid, "bid", price, delta_amount, amount);

            if amount == 0. {
                //info!("remove bid price={}", price);
                dinfo.bids.remove(&OrderedFloat(price));
            } else {
                //纠正 ask 盘口
                if dinfo.is_finish_snap && !dinfo.is_snaping {
                    correct(dinfo, sid, "bid", price);
                }

                //info!("update bid sid={} price={} amt={}",sid, price, amount);
                dinfo.bids.insert(OrderedFloat(price), amount);
            }
        }
        if v[COL_SIDEID] == 1. {
            //ask update
            let delta_amount: f64;

            match dinfo.asks.get(&OrderedFloat(price)) {
                Some(amount_in_price) => {
                    delta_amount = amount - amount_in_price;
                }
                None => {
                    delta_amount = amount;
                }
            }

            pending_adjust(sid, "ask", price, delta_amount, amount);

            if amount == 0. {
                //info!("remove ask price={}", price);
                dinfo.asks.remove(&OrderedFloat(price));
            } else {
                if dinfo.is_finish_snap && !dinfo.is_snaping {
                    correct(dinfo, sid, "ask", price);
                }

                //info!("update ask sid={} price={} amt={}",sid, price, amount);
                dinfo.asks.insert(OrderedFloat(price), amount);
            }
        }
    }

    if !dinfo.is_finish_snap {
        let warmup_s = PAIRMM_CONF.open_snapshot_warmup_s;
        if warmup_s > 0
            && dinfo.first_inc_ts_s > 0
            && ts_s - dinfo.first_inc_ts_s >= warmup_s
            && (ENGIN_CONF.stg == "pairmm_one_exchange_simple"
                || ENGIN_CONF.stg == "pairmm_two_exchange_simple"
                || ENGIN_CONF.stg == "pairmm")
        {
            if !dinfo.bids.is_empty() && !dinfo.asks.is_empty() {
                dinfo.is_finish_snap = true;
                dinfo.is_snaping = false;
            }
        }
    }

    if dinfo.is_finish_snap {
        if let Some((first_key, _first_value)) = dinfo.asks.first_key_value() {
            dinfo.ask1 = first_key.into_inner();
        }
        if let Some((last_key, _last_value)) = dinfo.bids.last_key_value() {
            dinfo.bid1 = last_key.into_inner();
        }
        //info!("dinfo sid={}({} {})  bid1={:?} ask1={:?}", sid, v[COL_PRICE], v[COL_AMOUNT], dinfo.bid1, dinfo.ask1);
    }
}
