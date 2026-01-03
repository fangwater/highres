use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookSide {
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    #[serde(default = "default_tick_size")]
    pub tick_size: f64,
    #[serde(default = "default_contract_value")]
    pub contract_value: f64,
}

fn default_tick_size() -> f64 {
    0.0
}

fn default_contract_value() -> f64 {
    1.0
}

#[derive(Debug, Clone)]
pub struct DepthInfo {
    pub sid: i32,
    pub is_snaping: bool,
    pub is_finish_snap: bool,
    pub bids: BTreeMap<OrderedFloat<f64>, f64>,
    pub asks: BTreeMap<OrderedFloat<f64>, f64>,
    pub bid1: f64,
    pub ask1: f64,
}

impl DepthInfo {
    pub fn new(sid: i32) -> Self {
        Self {
            sid,
            is_snaping: false,
            is_finish_snap: false,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            bid1: 0.0,
            ask1: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingItem {
    pub create_ts_us: i64,
    pub sid: i32,
    pub client_order_id: String,
    pub from_key: String,
    pub dup_key: String,
    pub target_sid: i32,
    pub price: f64,
    pub side: BookSide, // Bid (buy) / Ask (sell)
    pub amount: f64,
    pub amount_init: f64,
    pub inpos: f64,
    pub backlen: f64,
    pub tlen: f64,
    pub trade_consume_amount: f64,
}

#[derive(Debug, Clone, Default)]
pub struct PendingOrdersInPos {
    pub by_price: BTreeMap<OrderedFloat<f64>, Vec<PendingItem>>,
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub markets: HashMap<i32, MarketInfo>,
    pub depths: HashMap<i32, DepthInfo>,
    pub pending_bids: HashMap<i32, PendingOrdersInPos>,
    pub pending_asks: HashMap<i32, PendingOrdersInPos>,
    pub order_index: HashMap<String, (i32, BookSide, f64)>, // oid -> (sid, bid/ask, price)
    pub pos: HashMap<i32, f64>,
    pub open: f64,
}

impl EngineState {
    pub fn new(markets: HashMap<i32, MarketInfo>) -> Self {
        let mut depths = HashMap::new();
        let mut pending_bids = HashMap::new();
        let mut pending_asks = HashMap::new();
        let mut pos = HashMap::new();

        for sid in markets.keys().copied() {
            depths.insert(sid, DepthInfo::new(sid));
            pending_bids.insert(sid, PendingOrdersInPos::default());
            pending_asks.insert(sid, PendingOrdersInPos::default());
            pos.insert(sid, 0.0);
        }

        Self {
            markets,
            depths,
            pending_bids,
            pending_asks,
            order_index: HashMap::new(),
            pos,
            open: 0.0,
        }
    }

    pub fn is_known_sid(&self, sid: i32) -> bool {
        self.markets.contains_key(&sid)
    }

    pub fn market(&self, sid: i32) -> MarketInfo {
        self.markets.get(&sid).cloned().unwrap_or(MarketInfo {
            tick_size: 0.0,
            contract_value: 1.0,
        })
    }

    pub fn depth_mut(&mut self, sid: i32) -> anyhow::Result<&mut DepthInfo> {
        self.depths
            .get_mut(&sid)
            .ok_or_else(|| anyhow::anyhow!("unknown sid={sid}"))
    }
}

