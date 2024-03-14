use log::{info,debug};
use crate::trade::{TradeInfo, DepthInfo};
use crate::spending::{S_PENDING_PRICEKEY_BIDS, S_PENDING_PRICEKEY_ASKS};
use ordered_float::OrderedFloat;
use std::time::Duration;


//const COL_TSUS: usize   = 0;
const COL_IS: usize     = 1;
const COL_SIDEID: usize = 2;
const COL_PRICE: usize  = 3;
const COL_AMOUNT: usize = 4;
// const COL_TID: usize    = 5;
const COL_SID: usize    = 6;


pub fn pending_adjust(sid:i32, side:&str, price:f64, mut delta_amount:f64, amount_in_price:f64) {
    if side == "bid" {
        if let Some(mut porders) = S_PENDING_PRICEKEY_BIDS[&sid].try_write_for(Duration::from_secs(1)) {
            match porders.phash.get_mut(&OrderedFloat(price)) {
                Some(vs) => {
                    for v in vs {
                        info!("bid pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={} trade_comsume_amount={} bl={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price, v.trade_comsume_amount, v.backlen);
                        v.tlen = amount_in_price;
                        if v.trade_comsume_amount > 0. {
                            if v.inpos-v.trade_comsume_amount < 0. {
                                v.inpos = 0.;
                                delta_amount+=(v.inpos-v.trade_comsume_amount);
                            }
                            else {
                                v.inpos-=v.trade_comsume_amount;
                                delta_amount+=v.trade_comsume_amount;
                            }
                            
                            
                            v.trade_comsume_amount = 0.;
                        }
                        
                        if delta_amount > 0. {
                            v.backlen+=delta_amount;
                        }
                        else if delta_amount < 0. {
                            if v.inpos + v.backlen > 0. {
                                
                                v.inpos = v.inpos - (-delta_amount * (v.inpos / (v.inpos + v.backlen)));
                                if v.inpos < 0. {
                                        v.inpos = 0.
                                }
                            }
                            else {
                                v.inpos = 0.
                            }

                            if v.inpos + v.backlen > 0. {
                                v.backlen = v.backlen - (-delta_amount * (v.backlen / (v.inpos + v.backlen)));
                                if v.backlen < 0. {
                                    v.backlen = 0.
                                }
                            } else {
                                v.backlen = 0.
                            }
                        }
                        info!("finish bid pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price);

                    }
                }
                None => {}
            }
        } else {
            panic!("lock-err===== ");            
        }
    }
    else if side == "ask" {
        if let Some(mut porders) = S_PENDING_PRICEKEY_ASKS[&sid].try_write_for(Duration::from_secs(1)) {
            match porders.phash.get_mut(&OrderedFloat(price)) {
                Some(vs) => {
                    for v in vs {
                        info!("ask pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={} trade_comsume_amount={} bl={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price,v.trade_comsume_amount, v.backlen);
                        v.tlen = amount_in_price;
                        
                        if v.trade_comsume_amount > 0. {
                            if v.inpos-v.trade_comsume_amount < 0. {
                                v.inpos = 0.;
                                delta_amount+=(v.inpos-v.trade_comsume_amount);
                            }
                            else {
                                v.inpos-=v.trade_comsume_amount;
                                delta_amount+=v.trade_comsume_amount;
                            }
                        }

                        if delta_amount > 0. {
                            v.backlen+=delta_amount;

                        }
                        else if delta_amount < 0. {
                            if v.inpos + v.backlen > 0. {

                                v.inpos = v.inpos - (-delta_amount * (v.inpos / (v.inpos + v.backlen)));
                                if v.inpos < 0. {
                                    v.inpos = 0.
                                }
                            }
                            else {
                                v.inpos = 0.
                            }

                            if v.inpos + v.backlen > 0. {
                                v.backlen = v.backlen - (-delta_amount * (v.backlen / (v.inpos + v.backlen)));
                                if v.backlen < 0. {
                                    v.backlen = 0.
                                }
                            } else {
                                v.backlen = 0.
                            }
                        }
                        info!("finish ask pending[oid={} price={} amount={} inpos={}] adjust delta_amount={} amount_in_price={}", v.client_order_id, price, v.amount, v.inpos, delta_amount, amount_in_price);

                    }
                },
                None => {}
            }
        } else {
            panic!("lock-err===== ");            
        }
    } else {
        panic!("side={}", side);
    }
    
}

pub fn process(v:&Vec<f64>, tinfo:&mut TradeInfo) {
    let sid = v[COL_SID] as i32;
    let dinfo:&mut DepthInfo = tinfo.depths.get_mut(&sid).unwrap();
    let price = v[COL_PRICE];
    let amount = v[COL_AMOUNT];
    
    
    if dinfo.is_finish_snap {
        if let Some((first_key, _first_value)) = dinfo.asks.first_key_value() {
            dinfo.ask1 = first_key.into_inner();
        } 
        if let Some((last_key, _last_value)) = dinfo.bids.last_key_value() {
            dinfo.bid1 = last_key.into_inner();
        } 
        //info!("dinfo sid={}({} {})  bid1={:?} ask1={:?}", sid, v[COL_PRICE], v[COL_AMOUNT], dinfo.bid1, dinfo.ask1);
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
            debug!("insert (is) bid={} amt={}",v[COL_PRICE], v[COL_AMOUNT]);
            dinfo.bids.insert(OrderedFloat(price), amount);
        }
        if v[COL_SIDEID] == 1. {
            debug!("insert (is) ask={} amt={}",v[COL_PRICE], v[COL_AMOUNT]);
            dinfo.asks.insert(OrderedFloat(price), amount);
        }
    }
    else {
        if dinfo.is_snaping == true {
            dinfo.is_finish_snap = true;
            dinfo.is_snaping = false;
        }    
        
        if v[COL_SIDEID] == 0. {//bid update
            let delta_amount:f64;
            
            match dinfo.bids.get(&OrderedFloat(price)) {
                Some(amount_in_price) => {
                    delta_amount = amount - amount_in_price;
                },
                None => {
                    delta_amount = amount;
                }
            }
            
            pending_adjust(sid, "bid", price, delta_amount, amount);
            
            
            if amount == 0. {
                //info!("remove bid price={}", price);
                dinfo.bids.remove(&OrderedFloat(price));
            }
            else {       
                //info!("update bid sid={} price={} amt={}",sid, price, amount);
                dinfo.bids.insert(OrderedFloat(price), amount);
            }
            
        }
        if v[COL_SIDEID] == 1. {//ask update
            let delta_amount:f64;
            
            match dinfo.asks.get(&OrderedFloat(price)) {
                Some(amount_in_price) => {
                    delta_amount = amount - amount_in_price;
                },
                None => {
                    delta_amount = amount;
                }
            }
            
            pending_adjust(sid, "ask", price, delta_amount, amount);
            
            if amount == 0. {
                //info!("remove ask price={}", price);
                dinfo.asks.remove(&OrderedFloat(price));
            }
            else {       
                //info!("update ask sid={} price={} amt={}",sid, price, amount);
                dinfo.asks.insert(OrderedFloat(price), amount);
            }
        }
    }
}