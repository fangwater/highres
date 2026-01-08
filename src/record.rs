use std::fs::{create_dir_all, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use log::warn;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::gconf::ENGIN_CONF;
use csv::WriterBuilder;

const MARKET_IPC_PREFIX: &str = "/tmp/mth_pubs/";
const RECORD_IPC_PREFIX: &str = "/tmp/mth_pubs/stream_pairmm/";

struct RecordPublisher {
    _ctx: Arc<zmq::Context>,
    socket: zmq::Socket,
}

impl RecordPublisher {
    fn new(endpoint: &str) -> Result<Self, zmq::Error> {
        let ctx = Arc::new(zmq::Context::new());
        let socket = ctx.socket(zmq::PUB)?;
        socket.bind(endpoint)?;
        Ok(Self { _ctx: ctx, socket })
    }

    fn send(&self, topic: &[u8], payload: &[u8]) -> Result<(), zmq::Error> {
        self.socket.send(topic, zmq::SNDMORE)?;
        self.socket.send(payload, 0)?;
        Ok(())
    }
}

static RECORD_PUB: OnceCell<Mutex<RecordPublisher>> = OnceCell::new();


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDumpItem {
    pub create_ts:i64,
    pub update_ts:i64,
    pub client_order_id:String,
    pub symbol:String,
    pub ttype:String,
    pub sid:i32,
    pub side:String,
    pub price:f64,
    pub amount_init:f64,
    pub amount_update:f64,
    pub status:String,
    pub inpos:f64,
    pub tlen:f64,
    pub from_key:String,
    pub bid1:f64,
    pub ask1:f64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TSRecordItem {
    pub create_ts:i64,
    pub symbol:String,
    pub sid:i32,
    pub pos:f64,
    pub open:f64,
}

pub fn init_record_pub_from_market_ipc(market_ipc: &str) {
    if RECORD_PUB.get().is_some() {
        return;
    }
    let record_path = match record_path_from_market_ipc(market_ipc) {
        Some(path) => path,
        None => {
            warn!("record pub ipc path invalid: {}", market_ipc);
            return;
        }
    };
    if let Err(err) = init_record_pub(&record_path) {
        warn!("record pub init failed: {} err={}", record_path, err);
    }
}

fn record_path_from_market_ipc(market_ipc: &str) -> Option<String> {
    if !market_ipc.starts_with(MARKET_IPC_PREFIX) {
        return None;
    }
    let rest = &market_ipc[MARKET_IPC_PREFIX.len()..];
    Some(format!("{}{}", RECORD_IPC_PREFIX, rest))
}

fn init_record_pub(ipc_path: &str) -> Result<(), zmq::Error> {
    let endpoint = if ipc_path.starts_with("ipc://") {
        ipc_path.to_string()
    } else {
        format!("ipc://{}", ipc_path)
    };
    let ipc_fs_path = endpoint.strip_prefix("ipc://").unwrap_or(&endpoint);
    if let Some(parent) = Path::new(ipc_fs_path).parent() {
        let _ = create_dir_all(parent);
    }
    let publisher = RecordPublisher::new(&endpoint)?;
    let _ = RECORD_PUB.set(Mutex::new(publisher));
    Ok(())
}

pub fn write_to_csv(rd:&RecordDumpItem) {
    if let Some(publisher) = RECORD_PUB.get() {
        if let Ok(payload) = serde_json::to_vec(rd) {
            if publisher.lock().send(b"orders", &payload).is_ok() {
                return;
            }
        }
        warn!("record pub send failed, fallback to csv");
    }
    let dir = ENGIN_CONF.dump_path.to_string();
    let _ = create_dir_all(&dir);
    let file_name = dir + "/" + &rd.symbol + "_orders.csv";
   // let mut csv_writer = Writer::from_path(file_name).expect("Failed to create CSV writer");

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_name)
        .unwrap();

    let mut csv_writer = WriterBuilder::new().from_writer(file);
    //create_ts:pitem.create_ts,update_ts:update_ts_ms,client_order_id:pitem.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), sid:*sid, side:side.to_string(), price:pitem.price, amount_init:pitem.amount_init, amount_update:pitem.amount, status:status
    let _ = csv_writer.write_record(&[rd.client_order_id.clone(), rd.create_ts.to_string(),rd.update_ts.to_string(),rd.client_order_id.clone(),rd.symbol.to_string(),rd.ttype.to_string(),rd.sid.to_string(),rd.side.to_string(),rd.price.to_string(),rd.amount_init.to_string(),rd.amount_update.to_string(),rd.status.to_string(), rd.inpos.to_string(), rd.tlen.to_string(), rd.from_key.to_string(), rd.bid1.to_string(), rd.ask1.to_string()]);
    let _ = csv_writer.flush();
}


pub fn write_to_csv_ts(rd:&TSRecordItem) {
    if let Some(publisher) = RECORD_PUB.get() {
        if let Ok(payload) = serde_json::to_vec(rd) {
            if publisher.lock().send(b"nps", &payload).is_ok() {
                return;
            }
        }
        warn!("record pub send failed, fallback to csv");
    }
    let dir = ENGIN_CONF.dump_path.to_string();
    let _ = create_dir_all(&dir);
    let file_name = dir + "/" + &rd.symbol + "_nps.csv";
   // let mut csv_writer = Writer::from_path(file_name).expect("Failed to create CSV writer");

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_name)
        .unwrap();

    let mut csv_writer = WriterBuilder::new().from_writer(file);

    //create_ts:pitem.create_ts,update_ts:update_ts_ms,client_order_id:pitem.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), sid:*sid, side:side.to_string(), price:pitem.price, amount_init:pitem.amount_init, amount_update:pitem.amount, status:status
    let _ = csv_writer.write_record(&[rd.create_ts.to_string(), rd.symbol.to_string(), rd.sid.to_string(), rd.pos.to_string(), rd.open.to_string()]);
    let _ = csv_writer.flush();
}
