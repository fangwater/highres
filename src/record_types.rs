use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDumpItem {
    pub create_ts: i64,
    pub update_ts: i64,
    pub client_order_id: String,
    pub symbol: String,
    pub ttype: String,
    pub sid: i32,
    pub side: String,
    pub price: f64,
    pub amount_init: f64,
    pub amount_update: f64,
    pub status: String,
    pub inpos: f64,
    pub tlen: f64,
    pub from_key: String,
    pub bid1: f64,
    pub ask1: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSRecordItem {
    pub create_ts: i64,
    pub symbol: String,
    pub sid: i32,
    pub pos: f64,
    pub open: f64,
}
