#[path = "../record_types.rs"]
mod record_types;

use std::collections::HashMap;
use std::error::Error;
use std::fs::create_dir_all;
use std::path::Path;
use std::time::{Duration, Instant};

use log::{info, warn};
use rocksdb::{
    ColumnFamilyDescriptor, DBCompressionType, Direction, IteratorMode, Options, WriteOptions, DB,
};
use serde::Deserialize;

use crate::record_types::{RecordDumpItem, TSRecordItem};

const DEFAULT_IPC_PREFIX: &str = "ipc:///tmp/mth_pubs/stream_pairmm/okex-futures-binance-futures";
const DEFAULT_DB_ROOT: &str = "data/record_persist/pairmm/okex-futures-binance-futures";
const ZMQ_HWM: i32 = 100_000;

enum Mode {
    Run,
    Export(ExportArgs),
}

struct ExportArgs {
    symbol: String,
    kind: String,
    out: String,
    start_ts: Option<i64>,
    end_ts: Option<i64>,
    limit: Option<usize>,
}

struct Args {
    ipc_prefix: String,
    db_root: String,
    mode: Mode,
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args = parse_args();
    match args.mode {
        Mode::Run => run(&args.ipc_prefix, &args.db_root),
        Mode::Export(cfg) => export_csv(&args.db_root, &cfg),
    }
}

fn parse_args() -> Args {
    let mut ipc_prefix = DEFAULT_IPC_PREFIX.to_string();
    let mut db_root = DEFAULT_DB_ROOT.to_string();
    let mut export = false;
    let mut symbol: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut out: Option<String> = None;
    let mut start_ts: Option<i64> = None;
    let mut end_ts: Option<i64> = None;
    let mut limit: Option<usize> = None;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ipc-prefix" => {
                ipc_prefix = iter.next().unwrap_or_default();
            }
            "--db-root" => {
                db_root = iter.next().unwrap_or_default();
            }
            "--export" => {
                export = true;
            }
            "--symbol" => {
                symbol = iter.next();
            }
            "--kind" => {
                kind = iter.next();
            }
            "--out" => {
                out = iter.next();
            }
            "--start-ts" => {
                start_ts = iter.next().and_then(|v| v.parse::<i64>().ok());
            }
            "--end-ts" => {
                end_ts = iter.next().and_then(|v| v.parse::<i64>().ok());
            }
            "--limit" => {
                limit = iter.next().and_then(|v| v.parse::<usize>().ok());
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    let mode = if export {
        let symbol = symbol.unwrap_or_default();
        let kind = kind.unwrap_or_else(|| "orders".to_string());
        let out = out.unwrap_or_else(|| default_export_path(&symbol, &kind));
        Mode::Export(ExportArgs {
            symbol,
            kind,
            out,
            start_ts,
            end_ts,
            limit,
        })
    } else {
        Mode::Run
    };

    Args {
        ipc_prefix,
        db_root,
        mode,
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  stream_pairmm_record [--ipc-prefix <IPC>] [--db-root <DIR>]\n  stream_pairmm_record --export --symbol <SYMBOL> --kind <orders|nps> [--out <PATH>] [--start-ts <TS>] [--end-ts <TS>] [--limit <N>]"
    );
}

fn default_export_path(symbol: &str, kind: &str) -> String {
    if kind == "nps" {
        format!("{}_nps.csv", symbol)
    } else {
        format!("{}_orders.csv", symbol)
    }
}

fn run(ipc_prefix: &str, db_root: &str) -> Result<(), Box<dyn Error>> {
    let symbols = load_online_symbols()?;
    if symbols.is_empty() {
        warn!("online_symbols is empty in config.toml");
        return Ok(());
    }

    let prefix = normalize_ipc_prefix(ipc_prefix);
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::SUB)?;
    socket.set_rcvhwm(ZMQ_HWM)?;
    socket.set_subscribe(b"orders")?;
    socket.set_subscribe(b"nps")?;

    for symbol in symbols.iter() {
        let endpoint = format!("{}/{}.ipc", prefix, symbol);
        socket.connect(&endpoint)?;
        info!("subscribe {}", endpoint);
    }

    let mut store = RecordStore::new(db_root);
    let mut stats = RecvStats::new();
    loop {
        let parts = socket.recv_multipart(0)?;
        if parts.len() < 2 {
            warn!("invalid record message parts={}", parts.len());
            continue;
        }
        let topic = match std::str::from_utf8(&parts[0]) {
            Ok(v) => v,
            Err(_) => {
                warn!("invalid topic");
                continue;
            }
        };
        let payload = &parts[1];
        match topic {
            "orders" => match serde_json::from_slice::<RecordDumpItem>(payload) {
                Ok(item) => {
                    let ts = record_ts_orders(&item);
                    stats.on_orders(&item.symbol, ts);
                    if let Err(err) = store.put_orders(&item) {
                        warn!("persist orders failed: {}", err);
                    }
                }
                Err(err) => warn!("decode orders failed: {}", err),
            },
            "nps" => match serde_json::from_slice::<TSRecordItem>(payload) {
                Ok(item) => {
                    stats.on_nps(&item.symbol, item.create_ts);
                    if let Err(err) = store.put_nps(&item) {
                        warn!("persist nps failed: {}", err);
                    }
                }
                Err(err) => warn!("decode nps failed: {}", err),
            },
            _ => warn!("unknown topic {}", topic),
        }
    }
}

#[derive(Debug, Deserialize)]
struct OnlineSymbolsConf {
    online_symbols: Option<Vec<String>>,
}

fn load_online_symbols() -> Result<Vec<String>, Box<dyn Error>> {
    if !Path::new("config.toml").exists() {
        return Err("config.toml not found in current working directory".into());
    }
    let mut settings = config::Config::new();
    settings.merge(config::File::with_name("config.toml").required(true))?;
    let conf: OnlineSymbolsConf = settings.try_into()?;
    Ok(conf.online_symbols.unwrap_or_default())
}

fn normalize_ipc_prefix(raw: &str) -> String {
    let prefixed = if raw.starts_with("ipc://") {
        raw.to_string()
    } else {
        format!("ipc://{}", raw)
    };
    prefixed.trim_end_matches('/').to_string()
}

struct RecordStore {
    root: String,
    dbs: HashMap<String, DB>,
    seq: u64,
}

impl RecordStore {
    fn new(root: &str) -> Self {
        Self {
            root: root.trim_end_matches('/').to_string(),
            dbs: HashMap::new(),
            seq: 0,
        }
    }

    fn put_orders(&mut self, item: &RecordDumpItem) -> Result<(), Box<dyn Error>> {
        let ts = record_ts_orders(item);
        let key = self.next_key(ts);
        let payload = serde_json::to_vec(item)?;
        let db = self.open_db(&item.symbol)?;
        let cf = db
            .cf_handle("orders")
            .ok_or("column family orders missing")?;
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(false);
        db.put_cf_opt(cf, key.as_bytes(), payload, &write_opts)?;
        Ok(())
    }

    fn put_nps(&mut self, item: &TSRecordItem) -> Result<(), Box<dyn Error>> {
        let ts = item.create_ts;
        let key = self.next_key(ts);
        let payload = serde_json::to_vec(item)?;
        let db = self.open_db(&item.symbol)?;
        let cf = db.cf_handle("nps").ok_or("column family nps missing")?;
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(false);
        db.put_cf_opt(cf, key.as_bytes(), payload, &write_opts)?;
        Ok(())
    }

    fn open_db(&mut self, symbol: &str) -> Result<&DB, Box<dyn Error>> {
        if !self.dbs.contains_key(symbol) {
            let db_path = format!("{}/{}", self.root, symbol);
            let db = open_db(&db_path)?;
            self.dbs.insert(symbol.to_string(), db);
        }
        Ok(self.dbs.get(symbol).unwrap())
    }

    fn next_key(&mut self, ts: i64) -> String {
        let ts = if ts < 0 { 0 } else { ts };
        let key = format!("{:020}_{:020}", ts, self.seq);
        self.seq = self.seq.saturating_add(1);
        key
    }
}

struct RecvStats {
    last_log: Instant,
    orders_total: u64,
    nps_total: u64,
    orders_by_symbol: HashMap<String, u64>,
    nps_by_symbol: HashMap<String, u64>,
    last_orders_ts: Option<i64>,
    last_nps_ts: Option<i64>,
}

impl RecvStats {
    fn new() -> Self {
        Self {
            last_log: Instant::now(),
            orders_total: 0,
            nps_total: 0,
            orders_by_symbol: HashMap::new(),
            nps_by_symbol: HashMap::new(),
            last_orders_ts: None,
            last_nps_ts: None,
        }
    }

    fn on_orders(&mut self, symbol: &str, ts: i64) {
        self.orders_total += 1;
        *self.orders_by_symbol.entry(symbol.to_string()).or_insert(0) += 1;
        self.last_orders_ts = Some(ts);
        self.maybe_log();
    }

    fn on_nps(&mut self, symbol: &str, ts: i64) {
        self.nps_total += 1;
        *self.nps_by_symbol.entry(symbol.to_string()).or_insert(0) += 1;
        self.last_nps_ts = Some(ts);
        self.maybe_log();
    }

    fn maybe_log(&mut self) {
        if self.last_log.elapsed() < Duration::from_secs(3) {
            return;
        }
        let orders_detail = format_symbol_counts(&self.orders_by_symbol);
        let nps_detail = format_symbol_counts(&self.nps_by_symbol);
        let orders_ts = self
            .last_orders_ts
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        let nps_ts = self
            .last_nps_ts
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string());
        info!(
            "record recv agg interval=3s orders={} nps={} orders_last_ts={} nps_last_ts={} orders_by_symbol={} nps_by_symbol={}",
            self.orders_total,
            self.nps_total,
            orders_ts,
            nps_ts,
            orders_detail,
            nps_detail
        );
        self.orders_total = 0;
        self.nps_total = 0;
        self.orders_by_symbol.clear();
        self.nps_by_symbol.clear();
        self.last_orders_ts = None;
        self.last_nps_ts = None;
        self.last_log = Instant::now();
    }
}

fn format_symbol_counts(map: &HashMap<String, u64>) -> String {
    if map.is_empty() {
        return "-".to_string();
    }
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let mut parts = Vec::with_capacity(keys.len());
    for key in keys {
        if let Some(value) = map.get(key) {
            parts.push(format!("{}={}", key, value));
        }
    }
    parts.join(",")
}

fn record_ts_orders(item: &RecordDumpItem) -> i64 {
    if item.update_ts > 0 {
        item.update_ts
    } else {
        item.create_ts
    }
}

fn open_db(path: &str) -> Result<DB, Box<dyn Error>> {
    let path_ref = Path::new(path);
    if let Some(parent) = path_ref.parent() {
        if !parent.exists() {
            create_dir_all(parent)?;
        }
    }

    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);
    db_opts.set_compression_type(DBCompressionType::Lz4);

    let cf_opts = Options::default();
    let cfs = vec![
        ColumnFamilyDescriptor::new("default", cf_opts.clone()),
        ColumnFamilyDescriptor::new("orders", cf_opts.clone()),
        ColumnFamilyDescriptor::new("nps", cf_opts),
    ];

    Ok(DB::open_cf_descriptors(&db_opts, path_ref, cfs)?)
}

fn open_db_read_only(path: &str) -> Result<DB, Box<dyn Error>> {
    let mut db_opts = Options::default();
    db_opts.set_compression_type(DBCompressionType::Lz4);
    let cf_names = ["default", "orders", "nps"];
    Ok(DB::open_cf_for_read_only(&db_opts, path, cf_names, false)?)
}

fn export_csv(db_root: &str, cfg: &ExportArgs) -> Result<(), Box<dyn Error>> {
    if cfg.symbol.is_empty() {
        return Err("export requires --symbol".into());
    }
    let kind = cfg.kind.as_str();
    if kind != "orders" && kind != "nps" {
        return Err("export --kind must be orders or nps".into());
    }

    let db_path = format!("{}/{}", db_root.trim_end_matches('/'), cfg.symbol);
    let db = open_db_read_only(&db_path)?;
    let cf = db.cf_handle(kind).ok_or("column family missing")?;

    let start_key = cfg.start_ts.map(|ts| format!("{:020}_", ts).into_bytes());
    let iter = match start_key.as_ref() {
        Some(key) => db.iterator_cf(cf, IteratorMode::From(key, Direction::Forward)),
        None => db.iterator_cf(cf, IteratorMode::Start),
    };

    if let Some(parent) = Path::new(&cfg.out).parent() {
        let _ = create_dir_all(parent);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&cfg.out)?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(file);

    let mut written = 0usize;
    for item in iter {
        let (key, value) = item?;
        if let Some(end_ts) = cfg.end_ts {
            if let Some(ts) = parse_ts_from_key(&key) {
                if ts > end_ts {
                    break;
                }
            }
        }
        match kind {
            "orders" => {
                let rd: RecordDumpItem = serde_json::from_slice(&value)?;
                let _ = writer.write_record(&[
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
            }
            "nps" => {
                let rd: TSRecordItem = serde_json::from_slice(&value)?;
                let _ = writer.write_record(&[
                    rd.create_ts.to_string(),
                    rd.symbol.to_string(),
                    rd.sid.to_string(),
                    rd.pos.to_string(),
                    rd.open.to_string(),
                ]);
            }
            _ => {}
        }

        written += 1;
        if let Some(limit) = cfg.limit {
            if written >= limit {
                break;
            }
        }
    }
    writer.flush()?;
    info!("exported {} rows to {}", written, cfg.out);
    Ok(())
}

fn parse_ts_from_key(key: &[u8]) -> Option<i64> {
    if key.len() < 20 {
        return None;
    }
    let ts_str = std::str::from_utf8(&key[..20]).ok()?;
    ts_str.parse::<i64>().ok()
}
