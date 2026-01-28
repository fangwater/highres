#[path = "../gconf.rs"]
mod gconf;
#[path = "../record.rs"]
mod record;
#[path = "../pnlu_factor/mod.rs"]
mod pnlu_factor;

use std::collections::HashMap;
use std::error::Error;
use std::fs::create_dir_all;
use std::path::Path;

use csv::ReaderBuilder;
use log::{info, warn};
use rocksdb::{DBCompressionType, Direction, IteratorMode, Options, DB};
use serde::Deserialize;
use serde_json::json;

use crate::record::RecordDumpItem;
use crate::pnlu_factor::{FactorConfig, FactorState, OrderItem};

const DEFAULT_IPC_PREFIX: &str =
    "ipc:///tmp/mth_pubs/stream_pairmm/okex-futures-binance-futures";
const DEFAULT_DB_ROOT: &str = "data/record_persist/pairmm/okex-futures-binance-futures";
const DEFAULT_CONFIG_PATH: &str = "pnlu_factor.toml";

#[derive(Debug, Deserialize)]
struct FactorStreamConf {
    #[serde(default = "default_period_s")]
    period_s: i64,
    #[serde(default = "default_rolling_window")]
    rolling_window: usize,
    #[serde(default = "default_min_periods")]
    min_periods: usize,
    #[serde(default = "default_shift")]
    shift: usize,
    #[serde(default = "default_max_keep_periods")]
    max_keep_periods: usize,
    ipc_prefix: Option<String>,
    db_root: Option<String>,
    output_ipc_prefix: Option<String>,
    output_topic: Option<String>,
}

fn default_period_s() -> i64 {
    5
}

fn default_rolling_window() -> usize {
    720
}

fn default_min_periods() -> usize {
    300
}

fn default_shift() -> usize {
    120
}

fn default_max_keep_periods() -> usize {
    360
}

impl Default for FactorStreamConf {
    fn default() -> Self {
        Self {
            period_s: default_period_s(),
            rolling_window: default_rolling_window(),
            min_periods: default_min_periods(),
            shift: default_shift(),
            max_keep_periods: default_max_keep_periods(),
            ipc_prefix: None,
            db_root: None,
            output_ipc_prefix: None,
            output_topic: None,
        }
    }
}

struct Args {
    ipc_prefix: Option<String>,
    db_root: Option<String>,
    config_path: String,
    input_csv: Option<String>,
    csv_mode: CsvMode,
    tail_minutes: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CsvMode {
    All,
    Warmup,
    Live,
}

impl CsvMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "all" => Some(Self::All),
            "warmup" => Some(Self::Warmup),
            "live" => Some(Self::Live),
            _ => None,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args = parse_args();
    let mut conf = load_factor_conf(&args.config_path).unwrap_or_else(|err| {
        warn!("load {} failed: {}, using defaults", args.config_path, err);
        FactorStreamConf::default()
    });

    if conf.period_s <= 0 {
        return Err("period_s must be > 0".into());
    }
    if conf.rolling_window == 0 {
        return Err("rolling_window must be > 0".into());
    }
    if conf.max_keep_periods == 0 {
        return Err("max_keep_periods must be > 0".into());
    }
    if conf.min_periods > conf.rolling_window {
        conf.min_periods = conf.rolling_window;
    }

    let factor_conf = FactorConfig {
        period_s: conf.period_s,
        rolling_window: conf.rolling_window,
        min_periods: conf.min_periods,
        shift: conf.shift,
        max_keep_periods: conf.max_keep_periods,
    };
    info!(
        "factor config period_s={} rolling_window={} min_periods={} shift={} max_keep_periods={}",
        factor_conf.period_s,
        factor_conf.rolling_window,
        factor_conf.min_periods,
        factor_conf.shift,
        factor_conf.max_keep_periods
    );

    let output_endpoint = conf
        .output_ipc_prefix
        .clone()
        .unwrap_or_else(|| "ipc:///tmp/mth_pubs/pnlu_factor.ipc".to_string());
    if output_endpoint.trim().is_empty() {
        return Err("output_ipc_prefix is required".into());
    }
    let output_topic = conf
        .output_topic
        .clone()
        .unwrap_or_else(|| "pnlu_factor".to_string());
    let mut publisher = Publisher::new(&output_endpoint, &output_topic)?;

    if let Some(input_csv) = args.input_csv.as_ref() {
        info!(
            "csv mode input={} mode={:?} tail_minutes={}",
            input_csv, args.csv_mode, args.tail_minutes
        );
        return run_csv(
            input_csv,
            args.csv_mode,
            args.tail_minutes,
            &factor_conf,
            &mut publisher,
        );
    }

    let ipc_prefix = args
        .ipc_prefix
        .or_else(|| conf.ipc_prefix.clone())
        .unwrap_or_else(|| DEFAULT_IPC_PREFIX.to_string());
    let db_root = args
        .db_root
        .or_else(|| conf.db_root.clone())
        .unwrap_or_else(|| DEFAULT_DB_ROOT.to_string());

    let symbols = load_online_symbols()?;
    if symbols.is_empty() {
        return Err("online_symbols is empty in config.toml".into());
    }
    info!("stream mode symbols={}", symbols.len());

    let warmup_seconds =
        (factor_conf.rolling_window + factor_conf.shift) as i64 * factor_conf.period_s;

    let mut states = HashMap::new();
    for symbol in symbols.iter() {
        let mut state = FactorState::new(&factor_conf);
        warmup_symbol(&db_root, symbol, warmup_seconds, &mut state)?;
        states.insert(symbol.to_string(), state);
    }

    let prefix = normalize_ipc_prefix(&ipc_prefix);
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::SUB)?;
    socket.set_subscribe(b"orders")?;

    for symbol in symbols.iter() {
        let endpoint = format!("{}/{}.ipc", prefix, symbol);
        socket.connect(&endpoint)?;
        info!("subscribe {}", endpoint);
    }

    loop {
        let parts = socket.recv_multipart(0)?;
        if parts.len() < 2 {
            warn!("invalid message parts={}", parts.len());
            continue;
        }
        let topic = match std::str::from_utf8(&parts[0]) {
            Ok(v) => v,
            Err(_) => {
                warn!("invalid topic");
                continue;
            }
        };
        if topic != "orders" {
            continue;
        }
        let payload = &parts[1];
        let record = match serde_json::from_slice::<RecordDumpItem>(payload) {
            Ok(v) => v,
            Err(err) => {
                warn!("decode orders failed: {}", err);
                continue;
            }
        };
        let symbol = record.symbol.clone();
        let state = match states.get_mut(&symbol) {
            Some(v) => v,
            None => {
                warn!("symbol {} not configured, skip", symbol);
                continue;
            }
        };
        let item = order_item_from_record(&record);
        let rows = state.process_order(&item, true);
        let (open_cnt, close_cnt) = state.pending_counts();
        for row in rows {
            info!(
                "factor symbol={} ts={} target_ts={} pnlu_sum={:?} factor={:?} open_cnt={} close_cnt={}",
                symbol, row.ts, row.target_ts, row.pnlu_sum, row.factor, open_cnt, close_cnt
            );
            publisher.send(&symbol, &row)?;
        }
    }
}

fn parse_args() -> Args {
    let mut ipc_prefix = None;
    let mut db_root = None;
    let mut config_path = DEFAULT_CONFIG_PATH.to_string();
    let mut input_csv = None;
    let mut csv_mode = CsvMode::All;
    let mut tail_minutes = 10i64;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--ipc-prefix" => {
                ipc_prefix = iter.next();
            }
            "--db-root" => {
                db_root = iter.next();
            }
            "--config" => {
                if let Some(p) = iter.next() {
                    config_path = p;
                }
            }
            "--input-csv" => {
                input_csv = iter.next();
            }
            "--mode" => {
                if let Some(raw) = iter.next() {
                    if let Some(mode) = CsvMode::parse(&raw) {
                        csv_mode = mode;
                    }
                }
            }
            "--tail-minutes" => {
                if let Some(raw) = iter.next() {
                    if let Ok(v) = raw.parse::<i64>() {
                        tail_minutes = v;
                    }
                }
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    Args {
        ipc_prefix,
        db_root,
        config_path,
        input_csv,
        csv_mode,
        tail_minutes,
    }
}

fn print_usage() {
    eprintln!(
        "Usage:\n  pnlu_factor_stream [--ipc-prefix <IPC>] [--db-root <DIR>] [--config <PATH>]\n  pnlu_factor_stream --input-csv <PATH> [--mode <all|warmup|live>] [--tail-minutes <N>]"
    );
}

fn load_factor_conf(path: &str) -> Result<FactorStreamConf, Box<dyn Error>> {
    let mut settings = config::Config::new();
    settings.merge(config::File::with_name(path).required(true))?;
    let raw: config::Value = settings.get("pnlu_factor_stream")?;
    let conf: FactorStreamConf = raw.try_into()?;
    Ok(conf)
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

fn run_csv(
    input_csv: &str,
    mode: CsvMode,
    tail_minutes: i64,
    conf: &FactorConfig,
    publisher: &mut Publisher,
) -> Result<(), Box<dyn Error>> {
    let split_ts_ms = match mode {
        CsvMode::All => None,
        _ => {
            let max_ts = csv_max_ts_ms(input_csv)?;
            let split = max_ts.saturating_sub(tail_minutes.saturating_mul(60).saturating_mul(1000));
            info!(
                "csv split mode={:?} max_ts_ms={} split_ts_ms={}",
                mode, max_ts, split
            );
            Some(split)
        }
    };

    let mut states: HashMap<String, FactorState> = HashMap::new();

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .from_path(input_csv)?;

    for record in reader.records() {
        let record = record?;
        let (symbol, item) = match parse_csv_record(&record) {
            Some(v) => v,
            None => continue,
        };
        let ts_ms = item.ts_ms();
        let emit = match mode {
            CsvMode::All => true,
            CsvMode::Warmup => false,
            CsvMode::Live => split_ts_ms.map(|s| ts_ms > s).unwrap_or(true),
        };
        if mode == CsvMode::Warmup {
            if let Some(split) = split_ts_ms {
                if ts_ms > split {
                    continue;
                }
            }
        }
        let state = states
            .entry(symbol.clone())
            .or_insert_with(|| FactorState::new(conf));
        let rows = state.process_order(&item, emit);
        for row in rows {
            publisher.send(&symbol, &row)?;
        }
    }

    Ok(())
}

fn csv_max_ts_ms(path: &str) -> Result<i64, Box<dyn Error>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .from_path(path)?;
    let mut max_ts: Option<i64> = None;
    for record in reader.records() {
        let record = record?;
        let (_symbol, item) = match parse_csv_record(&record) {
            Some(v) => v,
            None => continue,
        };
        let ts = item.ts_ms();
        max_ts = Some(max_ts.map_or(ts, |v| v.max(ts)));
    }
    max_ts.ok_or_else(|| "csv is empty".into())
}

fn parse_csv_record(record: &csv::StringRecord) -> Option<(String, OrderItem)> {
    if record.len() < 17 {
        return None;
    }
    let symbol = record.get(4)?.to_string();
    let item = OrderItem {
        client_order_id: record.get(0)?.to_string(),
        create_ts: record.get(1)?.parse().ok()?,
        update_ts: record.get(2)?.parse().ok()?,
        sid: record.get(6)?.parse().ok()?,
        side: record.get(7)?.to_string(),
        price: record.get(8)?.parse().ok()?,
        amount_update: record.get(10)?.parse().ok()?,
        tlen: record.get(13)?.parse().ok()?,
        from_key: record.get(14)?.to_string(),
    };
    Some((symbol, item))
}

fn order_item_from_record(record: &RecordDumpItem) -> OrderItem {
    OrderItem {
        client_order_id: record.client_order_id.clone(),
        create_ts: record.create_ts,
        update_ts: record.update_ts,
        sid: record.sid,
        side: record.side.clone(),
        price: record.price,
        amount_update: record.amount_update,
        tlen: record.tlen,
        from_key: record.from_key.clone(),
    }
}

fn open_db_read_only(path: &str) -> Result<DB, Box<dyn Error>> {
    let mut db_opts = Options::default();
    db_opts.set_compression_type(DBCompressionType::Lz4);
    let cf_names = ["default", "orders", "nps"];
    Ok(DB::open_cf_for_read_only(
        &db_opts,
        path,
        cf_names,
        false,
    )?)
}

fn parse_ts_from_key(key: &[u8]) -> Option<i64> {
    if key.len() < 20 {
        return None;
    }
    let ts_str = std::str::from_utf8(&key[..20]).ok()?;
    ts_str.parse::<i64>().ok()
}

fn warmup_symbol(
    db_root: &str,
    symbol: &str,
    warmup_seconds: i64,
    state: &mut FactorState,
) -> Result<(), Box<dyn Error>> {
    let log_summary = |rows: usize, start_ms: Option<i64>, end_ms: Option<i64>, reason: &str| {
        let start_ms = start_ms.unwrap_or(0);
        let end_ms = end_ms.unwrap_or(0);
        let duration_s = if end_ms >= start_ms {
            (end_ms - start_ms) / 1000
        } else {
            0
        };
        info!(
            "warmup summary symbol={} rows={} duration_s={} start_ms={} end_ms={} reason={}",
            symbol, rows, duration_s, start_ms, end_ms, reason
        );
    };

    let db_path = format!("{}/{}", db_root.trim_end_matches('/'), symbol);
    if !Path::new(&db_path).exists() {
        warn!("db not found for {}, skip warmup", symbol);
        log_summary(0, None, None, "db_not_found");
        return Ok(());
    }
    let db = match open_db_read_only(&db_path) {
        Ok(db) => db,
        Err(err) => {
            warn!("open db {} failed: {}", db_path, err);
            log_summary(0, None, None, "open_db_failed");
            return Ok(());
        }
    };
    let cf = match db.cf_handle("orders") {
        Some(v) => v,
        None => {
            warn!("orders column family missing for {}", symbol);
            log_summary(0, None, None, "missing_orders_cf");
            return Ok(());
        }
    };

    let mut iter_end = db.iterator_cf(cf, IteratorMode::End);
    let end_key = match iter_end.next() {
        Some(Ok((key, _))) => key,
        _ => {
            log_summary(0, None, None, "no_orders");
            return Ok(());
        }
    };
    let end_ts_ms = match parse_ts_from_key(&end_key) {
        Some(v) => v,
        None => {
            warn!("invalid end key for {}, skip warmup", symbol);
            log_summary(0, None, None, "invalid_end_key");
            return Ok(());
        }
    };
    let warmup_ms = warmup_seconds.saturating_mul(1000);
    let start_ts_ms = end_ts_ms.saturating_sub(warmup_ms);
    let start_key = format!("{:020}_", start_ts_ms).into_bytes();

    let iter = db.iterator_cf(
        cf,
        IteratorMode::From(&start_key, Direction::Forward),
    );
    let mut rows = 0usize;
    for item in iter {
        let (key, value) = match item {
            Ok(v) => v,
            Err(err) => {
                warn!("db iterate error for {}: {}", symbol, err);
                continue;
            }
        };
        let ts = match parse_ts_from_key(&key) {
            Some(v) => v,
            None => continue,
        };
        if ts < start_ts_ms {
            continue;
        }
        if ts > end_ts_ms {
            break;
        }
        let record = match serde_json::from_slice::<RecordDumpItem>(&value) {
            Ok(v) => v,
            Err(err) => {
                warn!("decode warmup order failed: {}", err);
                continue;
            }
        };
        let item = order_item_from_record(&record);
        state.process_order(&item, false);
        rows += 1;
    }
    log_summary(rows, Some(start_ts_ms), Some(end_ts_ms), "ok");
    Ok(())
}

struct Publisher {
    socket: zmq::Socket,
    topic: Vec<u8>,
}

impl Publisher {
    fn new(endpoint: &str, topic: &str) -> Result<Self, Box<dyn Error>> {
        let ctx = zmq::Context::new();
        let socket = ctx.socket(zmq::PUB)?;
        let endpoint = normalize_ipc_prefix(endpoint);
        if let Some(path) = endpoint.strip_prefix("ipc://") {
            if let Some(parent) = Path::new(path).parent() {
                let _ = create_dir_all(parent);
            }
        }
        socket.bind(&endpoint)?;
        info!("publish {}", endpoint);
        Ok(Self {
            socket,
            topic: topic.as_bytes().to_vec(),
        })
    }

    fn send(&self, symbol: &str, row: &pnlu_factor::OutputRow) -> Result<(), Box<dyn Error>> {
        let payload = json!({
            "symbol": symbol,
            "ts": row.ts,
            "target_ts": row.target_ts,
            "pnlu_sum": row.pnlu_sum,
            "factor": row.factor,
        });
        let data = serde_json::to_vec(&payload)?;
        self.socket.send(&self.topic, zmq::SNDMORE)?;
        self.socket.send(data, 0)?;
        Ok(())
    }
}
