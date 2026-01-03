use crate::types::{BookSide, EngineState, TradeSide};
use ordered_float::OrderedFloat;

#[derive(Debug, Clone, Copy)]
pub struct TradeRow {
    pub ts_us: i64,
    pub sid: i32,
    pub side: TradeSide, // buy takes asks; sell takes bids
    pub price: f64,
    pub amount: f64,
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub ts_us: i64,
    pub sid: i32,
    pub client_order_id: String,
    pub maker_side: BookSide, // Bid (we bought) / Ask (we sold)
    pub price: f64,
    pub filled: f64,
    pub remaining: f64,
    pub is_full: bool,
    pub pos: f64,
    pub open: f64,
}

pub fn process(state: &mut EngineState, row: TradeRow) -> anyhow::Result<Vec<Fill>> {
    if !state.is_known_sid(row.sid) {
        return Ok(vec![]);
    }

    let contract_value = state.market(row.sid).contract_value;
    let (taker_hits, maker_side, price_predicate): (BookSide, BookSide, Box<dyn Fn(f64) -> bool>) =
        match row.side {
            TradeSide::Buy => (
                BookSide::Ask,
                BookSide::Ask,
                Box::new(move |pending_price| row.price >= pending_price),
            ),
            TradeSide::Sell => (
                BookSide::Bid,
                BookSide::Bid,
                Box::new(move |pending_price| row.price <= pending_price),
            ),
        };

    let pending = match taker_hits {
        BookSide::Bid => state.pending_bids.get_mut(&row.sid),
        BookSide::Ask => state.pending_asks.get_mut(&row.sid),
    };
    let Some(pending) = pending else {
        return Ok(vec![]);
    };

    let mut fills: Vec<Fill> = Vec::new();

    // Borrow disjoint fields to keep this single-threaded but borrow-checker friendly.
    let open = &mut state.open;
    let pos_map = &mut state.pos;
    let order_index = &mut state.order_index;

    let mut prices_to_remove: Vec<OrderedFloat<f64>> = Vec::new();
    for (price_key, orders_at_price) in pending.by_price.iter_mut() {
        let price = price_key.into_inner();
        if !price_predicate(price) {
            continue;
        }

        let mut idx_to_remove: Vec<usize> = Vec::new();
        for (idx, o) in orders_at_price.iter_mut().enumerate() {
            let pos = o.inpos - o.trade_consume_amount;
            let taker_amount = row.amount - pos;

            if taker_amount <= 0.0 {
                o.trade_consume_amount += row.amount;
                continue;
            }

            let fill_amount = taker_amount.min(o.amount).max(0.0);
            if fill_amount <= 0.0 {
                continue;
            }

            o.amount -= fill_amount;
            o.trade_consume_amount = o.trade_consume_amount + row.amount - fill_amount;

            match maker_side {
                BookSide::Bid => {
                    *open += fill_amount * contract_value;
                    *pos_map.entry(row.sid).or_insert(0.0) += fill_amount;
                }
                BookSide::Ask => {
                    *open -= fill_amount * contract_value;
                    *pos_map.entry(row.sid).or_insert(0.0) -= fill_amount;
                }
            }
            let pos_now = *pos_map.get(&row.sid).unwrap_or(&0.0);

            let is_full = o.amount <= 0.0;
            let fill = Fill {
                ts_us: row.ts_us,
                sid: row.sid,
                client_order_id: o.client_order_id.clone(),
                maker_side,
                price: o.price,
                filled: fill_amount,
                remaining: o.amount.max(0.0),
                is_full,
                pos: pos_now,
                open: *open,
            };
            fills.push(fill);

            if is_full {
                idx_to_remove.push(idx);
            }
        }

        // remove in reverse index order
        for idx in idx_to_remove.into_iter().rev() {
            let oid = orders_at_price[idx].client_order_id.clone();
            order_index.remove(&oid);
            orders_at_price.remove(idx);
        }

        if orders_at_price.is_empty() {
            prices_to_remove.push(*price_key);
        }
    }

    for p in prices_to_remove {
        pending.by_price.remove(&p);
    }

    Ok(fills)
}
