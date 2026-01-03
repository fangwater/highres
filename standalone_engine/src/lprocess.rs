use crate::types::{BookSide, DepthInfo, EngineState};
use ordered_float::OrderedFloat;

pub fn pending_adjust(
    state: &mut EngineState,
    sid: i32,
    side: BookSide,
    price: f64,
    mut delta_amount: f64,
    amount_in_price: f64,
) {
    let pending = match side {
        BookSide::Bid => state.pending_bids.get_mut(&sid),
        BookSide::Ask => state.pending_asks.get_mut(&sid),
    };
    let Some(pending) = pending else { return };

    let Some(orders) = pending.by_price.get_mut(&OrderedFloat(price)) else {
        return;
    };

    for o in orders.iter_mut() {
        o.tlen = amount_in_price;

        if o.trade_consume_amount > 0.0 {
            if o.inpos - o.trade_consume_amount < 0.0 {
                o.inpos = 0.0;
                delta_amount += o.inpos - o.trade_consume_amount;
            } else {
                o.inpos -= o.trade_consume_amount;
                delta_amount += o.trade_consume_amount;
            }
            o.trade_consume_amount = 0.0;
        }

        if delta_amount > 0.0 {
            o.backlen += delta_amount;
        } else if delta_amount < 0.0 {
            let denom = o.inpos + o.backlen;
            if denom > 0.0 {
                o.inpos = o.inpos - (-delta_amount * (o.inpos / denom));
                if o.inpos < 0.0 {
                    o.inpos = 0.0;
                }
            } else {
                o.inpos = 0.0;
            }

            let denom = o.inpos + o.backlen;
            if denom > 0.0 {
                o.backlen = o.backlen - (-delta_amount * (o.backlen / denom));
                if o.backlen < 0.0 {
                    o.backlen = 0.0;
                }
            } else {
                o.backlen = 0.0;
            }
        }
    }
}

pub fn correct_depth(dinfo: &mut DepthInfo, side: BookSide, price: f64) {
    let mut delist: Vec<f64> = Vec::new();

    match side {
        BookSide::Bid => {
            for (ask_price, _amount) in dinfo.asks.iter() {
                if price >= ask_price.into_inner() {
                    delist.push(ask_price.into_inner());
                } else {
                    break;
                }
            }
            for p in delist {
                dinfo.asks.remove(&OrderedFloat(p));
            }
        }
        BookSide::Ask => {
            for (bid_price, _amount) in dinfo.bids.iter().rev() {
                if price <= bid_price.into_inner() {
                    delist.push(bid_price.into_inner());
                } else {
                    break;
                }
            }
            for p in delist {
                dinfo.bids.remove(&OrderedFloat(p));
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OrderbookRow {
    pub ts_us: i64,
    pub sid: i32,
    pub is_snapshot: bool,
    pub side: BookSide,
    pub price: f64,
    pub amount: f64,
}

pub fn process(state: &mut EngineState, row: OrderbookRow) -> anyhow::Result<()> {
    if !state.is_known_sid(row.sid) {
        return Ok(());
    }

    if row.is_snapshot {
        {
            let dinfo = state.depth_mut(row.sid)?;
            if !dinfo.is_snaping {
                dinfo.bids.clear();
                dinfo.asks.clear();
            }
            dinfo.is_snaping = true;
            dinfo.is_finish_snap = false;

            match row.side {
                BookSide::Bid => {
                    dinfo.bids.insert(OrderedFloat(row.price), row.amount);
                }
                BookSide::Ask => {
                    dinfo.asks.insert(OrderedFloat(row.price), row.amount);
                }
            }
        }
    } else {
        let (prev, was_snaping, is_finish_snap) = {
            let dinfo = state.depth_mut(row.sid)?;
            let was_snaping = dinfo.is_snaping;
            if was_snaping {
                dinfo.is_finish_snap = true;
                dinfo.is_snaping = false;
            }

            let prev = match row.side {
                BookSide::Bid => dinfo.bids.get(&OrderedFloat(row.price)).copied().unwrap_or(0.0),
                BookSide::Ask => dinfo.asks.get(&OrderedFloat(row.price)).copied().unwrap_or(0.0),
            };
            (prev, was_snaping, dinfo.is_finish_snap)
        };

        let _ = was_snaping;
        let delta = row.amount - prev;
        pending_adjust(state, row.sid, row.side, row.price, delta, row.amount);

        {
            let dinfo = state.depth_mut(row.sid)?;
            match row.side {
                BookSide::Bid => {
                    if row.amount == 0.0 {
                        dinfo.bids.remove(&OrderedFloat(row.price));
                    } else {
                        if is_finish_snap && !dinfo.is_snaping {
                            correct_depth(dinfo, BookSide::Bid, row.price);
                        }
                        dinfo.bids.insert(OrderedFloat(row.price), row.amount);
                    }
                }
                BookSide::Ask => {
                    if row.amount == 0.0 {
                        dinfo.asks.remove(&OrderedFloat(row.price));
                    } else {
                        if is_finish_snap && !dinfo.is_snaping {
                            correct_depth(dinfo, BookSide::Ask, row.price);
                        }
                        dinfo.asks.insert(OrderedFloat(row.price), row.amount);
                    }
                }
            }

            if dinfo.is_finish_snap {
                if let Some((first_key, _)) = dinfo.asks.first_key_value() {
                    dinfo.ask1 = first_key.into_inner();
                }
                if let Some((last_key, _)) = dinfo.bids.last_key_value() {
                    dinfo.bid1 = last_key.into_inner();
                }
            }
        }
    }

    let _ = row.ts_us;
    Ok(())
}
