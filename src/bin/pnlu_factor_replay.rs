#[path = "../gconf.rs"]
mod gconf;
#[path = "../pnlu_factor/mod.rs"]
mod pnlu_factor;
#[path = "../record_types.rs"]
mod record_types;

use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use log::{info, warn};
use rocksdb::{DBCompressionType, Direction, IteratorMode, Options, DB};
use serde::Deserialize;

use crate::pnlu_factor::{FactorConfig, FactorState, OrderItem};
use crate::record_types::RecordDumpItem;

const DEFAULT_DB_ROOT: &str = "data/record_persist/pairmm/okex-futures-binance-futures";
const DEFAULT_CONFIG_PATH: &str = "pnlu_factor.toml";

struct Args {
    db_root: Option<String>,
    config_path: String,
    out_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FactorStreamConf {
    period_s: i64,
    rolling_window: usize,
    min_periods: usize,
    shift: usize,
    max_keep_periods: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args = parse_args()?;
    let mut conf = load_factor_conf(&args.config_path)?;
    let replay_conf = load_replay_conf(&args.config_path).unwrap_or(ReplayConf {
        out_dir: None,
        days: None,
        db_root: None,
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
    let mut days = replay_conf.days.ok_or("pnlu_factor_replay.days missing")?;
    if days > 7 {
        warn!("pnlu_factor_replay.days={} > 7, clamp to 7", days);
        days = 7;
    }
    if days <= 0 {
        return Err("pnlu_factor_replay.days must be > 0".into());
    }

    let symbols = load_online_symbols()?;
    if symbols.is_empty() {
        return Err("online_symbols is empty in config.toml".into());
    }
    info!("replay symbols={}", symbols.len());

    let out_dir = args
        .out_dir
        .clone()
        .or_else(|| replay_conf.out_dir.clone())
        .unwrap_or_else(|| ".".to_string());
    let db_root = args
        .db_root
        .clone()
        .or_else(|| replay_conf.db_root.clone())
        .unwrap_or_else(|| DEFAULT_DB_ROOT.to_string());
    info!(
        "replay config days={} db_root={} out_dir={} period_s={} rolling_window={} min_periods={} shift={} max_keep_periods={}",
        days,
        db_root,
        out_dir,
        factor_conf.period_s,
        factor_conf.rolling_window,
        factor_conf.min_periods,
        factor_conf.shift,
        factor_conf.max_keep_periods
    );
    let out_dir = out_dir.trim_end_matches('/').to_string();
    if !out_dir.is_empty() && !Path::new(&out_dir).exists() {
        std::fs::create_dir_all(&out_dir)?;
    }

    let conf = Arc::new(factor_conf);
    let db_root = Arc::new(db_root);
    let mut handles = Vec::with_capacity(symbols.len());

    for symbol in symbols {
        let symbol = symbol.clone();
        let conf = Arc::clone(&conf);
        let db_root = Arc::clone(&db_root);
        let out_dir = out_dir.clone();
        let handle = thread::spawn(move || {
            if let Err(err) = replay_symbol(&db_root, &out_dir, &symbol, days, &conf) {
                warn!("replay {} failed: {}", symbol, err);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut config_path = DEFAULT_CONFIG_PATH.to_string();
    let mut db_root = None;
    let mut out_dir = None;

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--db-root" => {
                db_root = iter.next();
            }
            "--config" => {
                if let Some(p) = iter.next() {
                    config_path = p;
                }
            }
            "--out-dir" => {
                out_dir = iter.next();
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
    }

    Ok(Args {
        db_root,
        config_path,
        out_dir,
    })
}

fn print_usage() {
    eprintln!("Usage:\n  pnlu_factor_replay [--db-root <DIR>] [--config <PATH>] [--out-dir <DIR>]");
}

#[derive(Debug, Deserialize)]
struct ReplayConf {
    out_dir: Option<String>,
    days: Option<i64>,
    db_root: Option<String>,
}

fn load_factor_conf(path: &str) -> Result<FactorStreamConf, Box<dyn Error>> {
    let mut settings = config::Config::new();
    settings.merge(config::File::with_name(path).required(true))?;
    let raw: config::Value = settings.get("pnlu_factor_stream")?;
    let conf: FactorStreamConf = raw.try_into()?;
    Ok(conf)
}

fn load_replay_conf(path: &str) -> Result<ReplayConf, Box<dyn Error>> {
    let mut settings = config::Config::new();
    settings.merge(config::File::with_name(path).required(true))?;
    let raw: config::Value = settings.get("pnlu_factor_replay")?;
    let conf: ReplayConf = raw.try_into()?;
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

fn open_db_read_only(path: &str) -> Result<DB, Box<dyn Error>> {
    let mut db_opts = Options::default();
    db_opts.set_compression_type(DBCompressionType::Lz4);
    let cf_names = ["default", "orders", "nps"];
    Ok(DB::open_cf_for_read_only(&db_opts, path, cf_names, false)?)
}

fn parse_ts_from_key(key: &[u8]) -> Option<i64> {
    if key.len() < 20 {
        return None;
    }
    let ts_str = std::str::from_utf8(&key[..20]).ok()?;
    ts_str.parse::<i64>().ok()
}

fn replay_symbol(
    db_root: &str,
    out_dir: &str,
    symbol: &str,
    days: i64,
    conf: &FactorConfig,
) -> Result<(), Box<dyn Error>> {
    let db_path = format!("{}/{}", db_root.trim_end_matches('/'), symbol);
    if !Path::new(&db_path).exists() {
        warn!("db not found for {}, skip", symbol);
        return Ok(());
    }
    let db = open_db_read_only(&db_path)?;
    let cf = match db.cf_handle("orders") {
        Some(v) => v,
        None => {
            warn!("orders column family missing for {}", symbol);
            return Ok(());
        }
    };

    let mut iter_end = db.iterator_cf(cf, IteratorMode::End);
    let end_key = match iter_end.next() {
        Some(Ok((key, _))) => key,
        _ => {
            warn!("no orders for {}", symbol);
            return Ok(());
        }
    };
    let mut iter_start = db.iterator_cf(cf, IteratorMode::Start);
    let start_key = match iter_start.next() {
        Some(Ok((key, _))) => key,
        _ => {
            warn!("no orders for {}", symbol);
            return Ok(());
        }
    };
    let end_ts_ms = match parse_ts_from_key(&end_key) {
        Some(v) => v,
        None => {
            warn!("invalid end key for {}", symbol);
            return Ok(());
        }
    };
    let earliest_ts_ms = match parse_ts_from_key(&start_key) {
        Some(v) => v,
        None => {
            warn!("invalid start key for {}", symbol);
            return Ok(());
        }
    };
    let days_ms = days.saturating_mul(24 * 60 * 60 * 1000);
    let mut start_ts_ms = end_ts_ms.saturating_sub(days_ms);
    let warmup_ms = (conf.rolling_window + conf.shift) as i64 * conf.period_s * 1000;
    if start_ts_ms < earliest_ts_ms {
        info!(
            "replay {} data span < days, using earliest_ts_ms={}",
            symbol, earliest_ts_ms
        );
        start_ts_ms = earliest_ts_ms;
    }
    let mut warmup_start_ms = start_ts_ms.saturating_sub(warmup_ms);
    if warmup_start_ms < earliest_ts_ms {
        warmup_start_ms = earliest_ts_ms;
    }
    info!(
        "replay {} earliest_ts_ms={} end_ts_ms={} start_ts_ms={} warmup_start_ms={}",
        symbol, earliest_ts_ms, end_ts_ms, start_ts_ms, warmup_start_ms
    );
    let start_key = format!("{:020}_", warmup_start_ms).into_bytes();
    let iter = db.iterator_cf(cf, IteratorMode::From(&start_key, Direction::Forward));

    let out_path = format!("{}/{}_pnlu_factor.csv", out_dir, symbol);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&out_path)?;
    let mut writer = csv::WriterBuilder::new()
        .has_headers(true)
        .from_writer(file);
    writer.write_record(&["symbol", "ts", "target_ts", "pnlu_sum", "factor"])?;

    let mut state = FactorState::new(conf);
    let start_ts_sec = start_ts_ms / 1000;
    let mut rows_out = 0usize;

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
        if ts < warmup_start_ms {
            continue;
        }
        if ts > end_ts_ms {
            break;
        }
        let record = match serde_json::from_slice::<RecordDumpItem>(&value) {
            Ok(v) => v,
            Err(err) => {
                warn!("decode order failed for {}: {}", symbol, err);
                continue;
            }
        };
        let order = order_item_from_record(&record);
        let rows = state.process_order(&order, true);
        for row in rows {
            if row.ts < start_ts_sec {
                continue;
            }
            writer.write_record(&[
                symbol.to_string(),
                row.ts.to_string(),
                row.target_ts.to_string(),
                format_opt_9(row.pnlu_sum),
                format_opt_9(row.factor),
            ])?;
            rows_out += 1;
        }
    }
    writer.flush()?;
    info!("replay {} rows={} output={}", symbol, rows_out, out_path);
    Ok(())
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

fn format_opt_9(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{:.9}", v),
        None => String::new(),
    }
}
