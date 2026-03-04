use std::fs::{create_dir_all, OpenOptions};
use std::path::Path;
use std::sync::Arc;

use log::warn;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;

use crate::gconf::ENGIN_CONF;
use csv::WriterBuilder;
#[path = "record_types.rs"]
mod record_types;
pub use record_types::{RecordDumpItem, TSRecordItem};

#[allow(dead_code)]
const MARKET_IPC_PREFIX: &str = "/tmp/mth_pubs/";
#[allow(dead_code)]
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
    let normalized = market_ipc
        .trim()
        .strip_prefix("ipc://")
        .unwrap_or(market_ipc.trim());
    let normalized = normalized.trim_end_matches('/');
    if !normalized.starts_with(MARKET_IPC_PREFIX) {
        return None;
    }
    let rest = &normalized[MARKET_IPC_PREFIX.len()..];
    if rest.is_empty() {
        return None;
    }
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

pub fn write_to_csv(rd: &RecordDumpItem) {
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
    let _ = csv_writer.write_record(&[
        rd.client_order_id.clone(),
        rd.create_ts.to_string(),
        rd.update_ts.to_string(),
        rd.client_order_id.clone(),
        rd.symbol.to_string(),
        rd.ttype.to_string(),
        rd.sid.to_string(),
        rd.side.to_string(),
        rd.price.to_string(),
        rd.amount_init.to_string(),
        rd.amount_update.to_string(),
        rd.status.to_string(),
        rd.inpos.to_string(),
        rd.tlen.to_string(),
        rd.from_key.to_string(),
        rd.bid1.to_string(),
        rd.ask1.to_string(),
    ]);
    let _ = csv_writer.flush();
}

pub fn write_to_csv_ts(rd: &TSRecordItem) {
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
    let _ = csv_writer.write_record(&[
        rd.create_ts.to_string(),
        rd.symbol.to_string(),
        rd.sid.to_string(),
        rd.pos.to_string(),
        rd.open.to_string(),
    ]);
    let _ = csv_writer.flush();
}
