use crate::lprocess::OrderbookRow;
use crate::tprocess::{Fill, TradeRow};
use crate::types::{BookSide, EngineState, MarketInfo, OrderSide, PendingItem, TradeSide};
use anyhow::Context;
use ordered_float::OrderedFloat;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Engine {
    pub state: EngineState,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub sid: i32,
    pub is_finish_snap: bool,
    pub bid1: f64,
    pub ask1: f64,
    pub pending_bids: usize,
    pub pending_asks: usize,
    pub pos: f64,
    pub open: f64,
}

impl Engine {
    pub fn new(markets: HashMap<i32, MarketInfo>) -> Self {
        Self {
            state: EngineState::new(markets),
        }
    }

    pub fn on_orderbook(&mut self, row: OrderbookRow) -> anyhow::Result<()> {
        crate::lprocess::process(&mut self.state, row)
    }

    pub fn on_trade(&mut self, row: TradeRow) -> anyhow::Result<Vec<Fill>> {
        crate::tprocess::process(&mut self.state, row)
    }

    pub fn add_own_order(
        &mut self,
        ts_us: i64,
        sid: i32,
        client_order_id: String,
        side: OrderSide,
        price: f64,
        amount: f64,
        from_key: String,
        dup_key: String,
        target_sid: i32,
    ) -> anyhow::Result<()> {
        if !self.state.is_known_sid(sid) {
            return Ok(());
        }

        let dinfo = self.state.depth_mut(sid)?;
        let maker_side = match side {
            OrderSide::Buy => BookSide::Bid,
            OrderSide::Sell => BookSide::Ask,
        };

        if dinfo.is_finish_snap {
            if maker_side == BookSide::Bid && dinfo.ask1 > 0.0 && price >= dinfo.ask1 {
                return Ok(());
            }
            if maker_side == BookSide::Ask && dinfo.bid1 > 0.0 && price <= dinfo.bid1 {
                return Ok(());
            }
        }

        let inpos = match maker_side {
            BookSide::Bid => dinfo.bids.get(&OrderedFloat(price)).copied().unwrap_or(0.0),
            BookSide::Ask => dinfo.asks.get(&OrderedFloat(price)).copied().unwrap_or(0.0),
        };

        let item = PendingItem {
            create_ts_us: ts_us,
            sid,
            client_order_id: client_order_id.clone(),
            from_key,
            dup_key,
            target_sid,
            price,
            side: maker_side,
            amount,
            amount_init: amount,
            inpos,
            backlen: 0.0,
            tlen: inpos,
            trade_consume_amount: 0.0,
        };

        let pending = match maker_side {
            BookSide::Bid => self.state.pending_bids.get_mut(&sid),
            BookSide::Ask => self.state.pending_asks.get_mut(&sid),
        }
        .context("pending map missing for sid")?;

        pending
            .by_price
            .entry(OrderedFloat(price))
            .or_default()
            .push(item);

        self.state.order_index.insert(client_order_id, (sid, maker_side, price));
        Ok(())
    }

    pub fn cancel_own_order(&mut self, client_order_id: &str) -> anyhow::Result<bool> {
        let Some((sid, side, price)) = self.state.order_index.remove(client_order_id) else {
            return Ok(false);
        };

        let pending = match side {
            BookSide::Bid => self.state.pending_bids.get_mut(&sid),
            BookSide::Ask => self.state.pending_asks.get_mut(&sid),
        };
        let Some(pending) = pending else { return Ok(false) };

        let Some(v) = pending.by_price.get_mut(&OrderedFloat(price)) else {
            return Ok(false);
        };

        if let Some(idx) = v.iter().position(|o| o.client_order_id == client_order_id) {
            v.remove(idx);
        }

        if v.is_empty() {
            pending.by_price.remove(&OrderedFloat(price));
        }

        Ok(true)
    }

    pub fn summary(&self, sid: i32) -> Option<Summary> {
        let d = self.state.depths.get(&sid)?;
        let pending_bids = self
            .state
            .pending_bids
            .get(&sid)
            .map(|p| p.by_price.values().map(|v| v.len()).sum())
            .unwrap_or(0);
        let pending_asks = self
            .state
            .pending_asks
            .get(&sid)
            .map(|p| p.by_price.values().map(|v| v.len()).sum())
            .unwrap_or(0);
        let pos = *self.state.pos.get(&sid).unwrap_or(&0.0);
        Some(Summary {
            sid,
            is_finish_snap: d.is_finish_snap,
            bid1: d.bid1,
            ask1: d.ask1,
            pending_bids,
            pending_asks,
            pos,
            open: self.state.open,
        })
    }

    pub fn clear(&mut self) {
        for sid in self.state.markets.keys().copied() {
            if let Some(p) = self.state.pending_bids.get_mut(&sid) {
                p.by_price.clear();
            }
            if let Some(p) = self.state.pending_asks.get_mut(&sid) {
                p.by_price.clear();
            }
            if let Some(d) = self.state.depths.get_mut(&sid) {
                d.bids.clear();
                d.asks.clear();
                d.bid1 = 0.0;
                d.ask1 = 0.0;
                d.is_finish_snap = false;
                d.is_snaping = false;
            }
            self.state.pos.insert(sid, 0.0);
        }
        self.state.order_index.clear();
        self.state.open = 0.0;
    }

    pub fn on_tick(&mut self, _ts_ms: i64) {
        // placeholder: user can hook strategy/sampling here
    }
}

pub fn trade_row(ts_us: i64, sid: i32, side: TradeSide, price: f64, amount: f64) -> TradeRow {
    TradeRow {
        ts_us,
        sid,
        side,
        price,
        amount,
    }
}

pub fn orderbook_row(
    ts_us: i64,
    sid: i32,
    is_snapshot: bool,
    side: BookSide,
    price: f64,
    amount: f64,
) -> OrderbookRow {
    OrderbookRow {
        ts_us,
        sid,
        is_snapshot,
        side,
        price,
        amount,
    }
}

