use std::fs::create_dir_all;
use std::path::Path;
use std::sync::Arc;

use log::warn;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;

const ZMQ_HWM: i32 = 100_000;

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
        socket.set_sndhwm(ZMQ_HWM)?;
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
    let payload = match serde_json::to_vec(rd) {
        Ok(v) => v,
        Err(err) => {
            warn!("record serialize orders failed: {}", err);
            return;
        }
    };
    match RECORD_PUB.get() {
        Some(publisher) => {
            if let Err(err) = publisher.lock().send(b"orders", &payload) {
                warn!("record pub send orders failed: {}", err);
            }
        }
        None => warn!("record pub not initialized, drop orders record"),
    }
}

pub fn write_to_csv_ts(rd: &TSRecordItem) {
    let payload = match serde_json::to_vec(rd) {
        Ok(v) => v,
        Err(err) => {
            warn!("record serialize nps failed: {}", err);
            return;
        }
    };
    match RECORD_PUB.get() {
        Some(publisher) => {
            if let Err(err) = publisher.lock().send(b"nps", &payload) {
                warn!("record pub send nps failed: {}", err);
            }
        }
        None => warn!("record pub not initialized, drop nps record"),
    }
}
