use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use config::{Config, File};
use log::{info, warn};
use redis::Commands;
use serde::{Deserialize, Serialize};

const DEFAULT_PROCESS_CONFIG: &str = "pnlu_factor_rolling.toml";
const DEFAULT_SYMBOL_CONFIG: &str = "pnlu_factor_rolling_symbols.json";
const DEFAULT_RELOAD_SEC: u64 = 5;
const DEFAULT_TOPIC: &str = "pnlu_factor";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SymbolConfigPartial {
    rolling_window: Option<usize>,
    min_periods: Option<usize>,
    quantiles: Option<Vec<f64>>,
    csv_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SymbolConfigFile {
    default: Option<SymbolConfigPartial>,
    symbols: Option<HashMap<String, SymbolConfigPartial>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProcessConfigFile {
    csv_dir: Option<String>,
    ipc_prefix: Option<String>,
    reload_sec: Option<u64>,
    symbols_config: Option<String>,
    log_factor_thresholds: Option<bool>,
    redis_url: Option<String>,
    redis_key: Option<String>,
}

#[derive(Debug, Clone)]
struct SymbolConfig {
    rolling_window: usize,
    min_periods: usize,
    quantiles: Vec<f64>,
    csv_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct SymbolState {
    config: SymbolConfig,
    values: VecDeque<f64>,
    last_quantiles: Vec<f64>,
    ready: bool,
}

#[derive(Debug, Deserialize)]
struct FactorMsg {
    symbol: String,
    factor: Option<f64>,
    ts: Option<i64>,
    target_ts: Option<i64>,
}

struct RedisWriter {
    client: redis::Client,
    key_suffix: String,
    conn: Option<redis::Connection>,
}

impl RedisWriter {
    fn connect(url: &str, key: &str) -> Result<Self, Box<dyn Error>> {
        let client = redis::Client::open(url)?;
        let conn = Some(client.get_connection()?);
        Ok(Self {
            client,
            key_suffix: key.to_string(),
            conn,
        })
    }

    fn write_json(
        &mut self,
        symbol: &str,
        payload: &serde_json::Value,
    ) -> Result<(), Box<dyn Error>> {
        let body = serde_json::to_string(payload)?;
        if self.conn.is_none() {
            self.conn = self.client.get_connection().ok();
        }
        let key = format!("{}{}", symbol, self.key_suffix);
        if let Some(conn) = self.conn.as_mut() {
            let res: redis::RedisResult<String> = conn.set(&key, body);
            match res {
                Ok(_) => Ok(()),
                Err(err) => {
                    self.conn = None;
                    Err(err.into())
                }
            }
        } else {
            Err("redis connection unavailable".into())
        }
    }
}

fn default_partial(cfg: &SymbolConfigFile) -> SymbolConfigPartial {
    cfg.default.clone().unwrap_or(SymbolConfigPartial {
        rolling_window: Some(100000),
        min_periods: Some(10000),
        quantiles: Some(vec![0.1, 0.5, 0.9]),
        csv_path: None,
    })
}

fn ensure_symbols_config(
    path: &str,
    cfg: &mut SymbolConfigFile,
    symbols: &[String],
) -> Result<bool, Box<dyn Error>> {
    let mut changed = false;
    let default = default_partial(cfg);
    let overrides = cfg.symbols.get_or_insert_with(HashMap::new);
    for symbol in symbols.iter() {
        if !overrides.contains_key(symbol) {
            overrides.insert(symbol.clone(), default.clone());
            changed = true;
        }
    }
    if changed {
        let data = serde_json::to_string_pretty(cfg)?;
        fs::write(path, format!("{}\n", data))?;
    }
    Ok(changed)
}

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let process_cfg = load_process_config(DEFAULT_PROCESS_CONFIG)?;
    let symbols_config_path = process_cfg
        .symbols_config
        .clone()
        .unwrap_or_else(|| DEFAULT_SYMBOL_CONFIG.to_string());
    let reload_sec = process_cfg.reload_sec.unwrap_or(DEFAULT_RELOAD_SEC).max(1);
    let log_factor_thresholds = process_cfg.log_factor_thresholds.unwrap_or(false);
    let redis_url = process_cfg.redis_url.clone().unwrap_or_default();
    let redis_key = process_cfg
        .redis_key
        .clone()
        .unwrap_or_else(|| "_pnlu_factor_thresholds".to_string());
    let ipc_prefix = process_cfg.ipc_prefix.clone().unwrap_or_default();
    let csv_dir = process_cfg
        .csv_dir
        .clone()
        .unwrap_or_else(|| "/mnt/data/pnlu_factor_replay".to_string());

    let mut cfg = load_symbol_config(&symbols_config_path)?;
    let all_symbols = load_online_symbols()?;
    if all_symbols.is_empty() {
        return Err("online_symbols is empty in config.toml".into());
    }
    validate_overrides(&cfg, &all_symbols)?;
    let changed = ensure_symbols_config(&symbols_config_path, &mut cfg, &all_symbols)?;
    if changed {
        info!("symbols config expanded {}", symbols_config_path);
    }
    info!(
        "rolling-metrics config symbols={} ipc_endpoint={} reload_sec={} csv_dir={} symbols_config={}",
        all_symbols.len(),
        ipc_prefix,
        reload_sec,
        csv_dir,
        symbols_config_path
    );

    let mut states = init_states(&cfg, &all_symbols, &csv_dir)?;
    warmup_from_csv(&mut states)?;

    if ipc_prefix.trim().is_empty() {
        return Err("ipc_prefix is required for pnlu_factor_rolling_metrics".into());
    }
    if redis_url.trim().is_empty() {
        return Err("redis_url is required for pnlu_factor_rolling_metrics".into());
    }

    let endpoint = normalize_ipc_prefix(&ipc_prefix);
    let ctx = zmq::Context::new();
    let socket = ctx.socket(zmq::SUB)?;
    socket.set_subscribe(DEFAULT_TOPIC.as_bytes())?;
    socket.set_rcvtimeo(1000)?;
    socket.connect(&endpoint)?;
    info!("subscribe {}", endpoint);

    let mut redis_writer =
        RedisWriter::connect(&redis_url, &redis_key).unwrap_or_else(|err| {
            panic!("redis connect failed url={} err={}", redis_url, err)
        });
    info!(
        "redis connected url={} key_suffix={}",
        redis_url, redis_key
    );

    let mut last_reload = Instant::now();
    let mut last_mtime = file_mtime(&symbols_config_path).ok();

    loop {
        match socket.recv_multipart(0) {
            Ok(parts) => {
                if parts.len() < 2 {
                    continue;
                }
                let topic = match std::str::from_utf8(&parts[0]) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                if topic != DEFAULT_TOPIC {
                    continue;
                }
                let payload = &parts[1];
                let msg = match serde_json::from_slice::<FactorMsg>(payload) {
                    Ok(v) => v,
                    Err(err) => {
                        warn!("decode factor msg failed: {}", err);
                        continue;
                    }
                };
                let state = match states.get_mut(&msg.symbol) {
                    Some(v) => v,
                    None => {
                        return Err(format!(
                            "symbol {} not configured in pnlu_factor_rolling_metrics",
                            msg.symbol
                        )
                        .into());
                    }
                };
                if msg.symbol.to_ascii_uppercase().contains("BTC") {
                    info!("btc factor msg symbol={} factor={:?}", msg.symbol, msg.factor);
                }
                if let Some(value) = msg.factor {
                    push_value(state, value);
                    update_quantiles(state);
                    let payload = serde_json::json!({
                        "symbol": msg.symbol,
                        "ts": msg.ts,
                        "target_ts": msg.target_ts,
                        "factor": value,
                        "quantiles": state.config.quantiles,
                        "thresholds": state.last_quantiles,
                        "ready": state.ready,
                    });
                    if log_factor_thresholds {
                        info!("{}", payload);
                    }
                    if let Err(err) = redis_writer.write_json(&msg.symbol, &payload) {
                        warn!("redis write failed: {}", err);
                    }
                }
            }
            Err(err) => {
                if err == zmq::Error::EAGAIN {
                    // timeout
                } else {
                    warn!("zmq recv failed: {}", err);
                }
            }
        }

        if last_reload.elapsed() >= Duration::from_secs(reload_sec) {
            last_reload = Instant::now();
            if let Ok(mtime) = file_mtime(&symbols_config_path) {
                if last_mtime.map(|v| v < mtime).unwrap_or(true) {
                    last_mtime = Some(mtime);
                    match load_symbol_config(&symbols_config_path) {
                        Ok(mut new_cfg) => {
                            validate_overrides(&new_cfg, &all_symbols)?;
                            if ensure_symbols_config(&symbols_config_path, &mut new_cfg, &all_symbols)? {
                                last_mtime = file_mtime(&symbols_config_path).ok();
                            }
                            let _added = apply_config(
                                &mut cfg,
                                new_cfg,
                                &mut states,
                                &all_symbols,
                                &csv_dir,
                            )?;
                            info!("reloaded config {}", symbols_config_path);
                        }
                        Err(err) => warn!("reload config failed: {}", err),
                    }
                }
            }
        }
    }
}

fn load_symbol_config(path: &str) -> Result<SymbolConfigFile, Box<dyn Error>> {
    let raw = fs::read_to_string(path)?;
    let cfg: SymbolConfigFile = serde_json::from_str(&raw)?;
    Ok(cfg)
}

fn load_process_config(path: &str) -> Result<ProcessConfigFile, Box<dyn Error>> {
    let mut settings = Config::new();
    settings.merge(File::with_name(path).required(true))?;
    let conf: ProcessConfigFile = settings.try_into()?;
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
    let mut settings = Config::new();
    settings.merge(File::with_name("config.toml").required(true))?;
    let conf: OnlineSymbolsConf = settings.try_into()?;
    Ok(conf.online_symbols.unwrap_or_default())
}

fn validate_overrides(
    cfg: &SymbolConfigFile,
    symbols: &[String],
) -> Result<(), Box<dyn Error>> {
    if let Some(overrides) = cfg.symbols.as_ref() {
        let set: HashSet<&str> = symbols.iter().map(|s| s.as_str()).collect();
        for symbol in overrides.keys() {
            if !set.contains(symbol.as_str()) {
                return Err(format!("symbol {} not in config.toml online_symbols", symbol).into());
            }
        }
    }
    Ok(())
}

fn init_states(
    cfg: &SymbolConfigFile,
    symbols: &[String],
    csv_dir: &str,
) -> Result<HashMap<String, SymbolState>, Box<dyn Error>> {
    let mut states = HashMap::new();
    let overrides = cfg.symbols.as_ref();
    for symbol in symbols.iter() {
        let partial = overrides.and_then(|map| map.get(symbol));
        let config = build_symbol_config(cfg, csv_dir, symbol, partial);
        states.insert(
            symbol.to_string(),
            SymbolState {
                config,
                values: VecDeque::new(),
                last_quantiles: Vec::new(),
                ready: false,
            },
        );
    }
    Ok(states)
}

fn build_symbol_config(
    cfg: &SymbolConfigFile,
    csv_dir: &str,
    symbol: &str,
    partial: Option<&SymbolConfigPartial>,
) -> SymbolConfig {
    let default = cfg.default.clone().unwrap_or(SymbolConfigPartial {
        rolling_window: Some(100000),
        min_periods: Some(10000),
        quantiles: Some(vec![0.1, 0.5, 0.9]),
        csv_path: None,
    });
    let rolling_window = partial
        .and_then(|v| v.rolling_window)
        .or(default.rolling_window)
        .unwrap_or(720)
        .max(1);
    let min_periods = partial
        .and_then(|v| v.min_periods)
        .or(default.min_periods)
        .unwrap_or(1)
        .min(rolling_window)
        .max(1);
    let quantiles = normalize_quantiles(
        partial
            .and_then(|v| v.quantiles.clone())
            .or(default.quantiles)
            .unwrap_or_default(),
    );
    let csv_path = partial
        .and_then(|v| v.csv_path.clone())
        .or(default.csv_path)
        .or_else(|| Some(format!("{}/{}_pnlu_factor.csv", csv_dir, symbol)));
    SymbolConfig {
        rolling_window,
        min_periods,
        quantiles,
        csv_path: csv_path.map(PathBuf::from),
    }
}

fn warmup_from_csv(states: &mut HashMap<String, SymbolState>) -> Result<(), Box<dyn Error>> {
    for (symbol, state) in states.iter_mut() {
        let path = match state.config.csv_path.as_ref() {
            Some(p) => p.clone(),
            None => {
                warn!("symbol {} csv_path missing, skip warmup", symbol);
                continue;
            }
        };
        if !path.exists() {
            warn!("symbol {} csv not found {}", symbol, path.display());
            continue;
        }
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_path(&path)?;
        let mut rows = 0usize;
        for record in reader.records() {
            let record = record?;
            if record.len() < 5 {
                continue;
            }
            let factor = record.get(4).and_then(|v| v.parse::<f64>().ok());
            if let Some(value) = factor {
                push_value(state, value);
            }
            rows += 1;
        }
        update_quantiles(state);
        info!(
            "warmup {} rows={} window={} min_periods={}",
            symbol, rows, state.config.rolling_window, state.config.min_periods
        );
    }
    Ok(())
}

fn push_value(state: &mut SymbolState, value: f64) {
    state.values.push_back(value);
    while state.values.len() > state.config.rolling_window {
        state.values.pop_front();
    }
}

fn compute_quantiles(values: &VecDeque<f64>, quantiles: &[f64]) -> Vec<f64> {
    let mut data: Vec<f64> = values.iter().copied().collect();
    data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(a.total_cmp(b)));
    if data.is_empty() {
        return vec![];
    }
    let n = data.len() as f64;
    let mut out = Vec::with_capacity(quantiles.len());
    for &q in quantiles {
        let q = q.clamp(0.0, 1.0);
        let pos = q * (n - 1.0);
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;
        if lo == hi {
            out.push(data[lo]);
        } else {
            let w = pos - lo as f64;
            out.push(data[lo] * (1.0 - w) + data[hi] * w);
        }
    }
    out
}

fn normalize_quantiles(raw: Vec<f64>) -> Vec<f64> {
    let mut qs: Vec<f64> = raw
        .into_iter()
        .filter(|v| v.is_finite())
        .collect();
    qs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(a.total_cmp(b)));
    qs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    qs
}

fn normalize_ipc_prefix(raw: &str) -> String {
    let prefixed = if raw.starts_with("ipc://") {
        raw.to_string()
    } else {
        format!("ipc://{}", raw)
    };
    prefixed.trim_end_matches('/').to_string()
}

fn file_mtime(path: &str) -> Result<SystemTime, Box<dyn Error>> {
    Ok(fs::metadata(path)?.modified()?)
}

fn apply_config(
    current: &mut SymbolConfigFile,
    new_cfg: SymbolConfigFile,
    states: &mut HashMap<String, SymbolState>,
    symbols: &[String],
    csv_dir: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let before: HashSet<String> = states.keys().cloned().collect();
    *current = new_cfg.clone();
    let mut next_states = HashMap::new();
    let overrides = new_cfg.symbols.as_ref();
    for symbol in symbols.iter() {
        let partial = overrides.and_then(|map| map.get(symbol));
        let config = build_symbol_config(&new_cfg, csv_dir, symbol, partial);
        let mut state = states.remove(symbol).unwrap_or(SymbolState {
            config: config.clone(),
            values: VecDeque::new(),
            last_quantiles: Vec::new(),
            ready: false,
        });
        state.config = config.clone();
        while state.values.len() > state.config.rolling_window {
            state.values.pop_front();
        }
        if state.values.is_empty() {
            if let Some(path) = state.config.csv_path.clone() {
                if path.exists() {
                    let mut reader = csv::ReaderBuilder::new()
                        .has_headers(true)
                        .from_path(&path)?;
                    for record in reader.records() {
                        let record = record?;
                        if record.len() < 5 {
                            continue;
                        }
                        if let Some(value) = record.get(4).and_then(|v| v.parse::<f64>().ok()) {
                            push_value(&mut state, value);
                        }
                    }
                }
            }
        }
        update_quantiles(&mut state);
        next_states.insert(symbol.to_string(), state);
    }
    let after: HashSet<String> = next_states.keys().cloned().collect();
    let added = after.difference(&before).cloned().collect::<Vec<_>>();
    *states = next_states;
    Ok(added)
}

fn update_quantiles(state: &mut SymbolState) {
    let count = state.values.len();
    let ready = count >= state.config.min_periods;
    if ready {
        state.last_quantiles = compute_quantiles(&state.values, &state.config.quantiles);
    } else {
        state.last_quantiles.clear();
    }
    state.ready = ready;
}
