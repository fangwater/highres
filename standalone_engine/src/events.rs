use crate::types::{BookSide, MarketInfo, OrderSide, TradeSide};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum InEvent {
    Init {
        markets: HashMap<i32, MarketInfo>,
    },

    // Orderbook snapshot/incremental row
    Ob {
        ts_us: i64,
        sid: i32,
        is_snapshot: bool,
        side: BookSide,
        price: f64,
        amount: f64,
    },

    // Trade tick row
    Trade {
        ts_us: i64,
        sid: i32,
        side: TradeSide,
        price: f64,
        amount: f64,
    },

    // Your own order feed (maker order)
    OwnNew {
        ts_us: i64,
        sid: i32,
        client_order_id: String,
        side: OrderSide,
        price: f64,
        amount: f64,
        #[serde(default)]
        from_key: String,
        #[serde(default)]
        dup_key: String,
        #[serde(default)]
        target_sid: i32,
    },

    OwnCancel {
        client_order_id: String,
    },

    Tick {
        ts_ms: i64,
    },

    Sample {},

    Clear {},
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum OutEvent {
    Fill {
        ts_us: i64,
        sid: i32,
        client_order_id: String,
        maker_side: BookSide,
        price: f64,
        filled: f64,
        remaining: f64,
        is_full: bool,
        pos: f64,
        open: f64,
    },
    Summary {
        sid: i32,
        is_finish_snap: bool,
        bid1: f64,
        ask1: f64,
        pending_bids: usize,
        pending_asks: usize,
        pos: f64,
        open: f64,
    },
    Ack {
        ok: bool,
        msg: String,
    },
}

