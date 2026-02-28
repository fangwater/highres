use std::collections::{HashMap, HashSet, VecDeque};
use std::error::Error;
use std::fs;
use std::time::{Duration, Instant, SystemTime};

use config::{Config, File};
use log::{info, warn};
use redis::Commands;
use serde::{Deserialize, Serialize};

const DEFAULT_SYMBOL_CONFIG: &str = "pnlu_factor_rolling_symbols.json";
const DEFAULT_RELOAD_SEC: u64 = 5;

#[derive(Debug, Clone, Deserialize)]
pub struct RollingProcessConfigFile {
    pub symbols_config: Option<String>,
    pub reload_sec: Option<u64>,
    pub log_factor_thresholds: Option<bool>,
    pub log_redis_write_success: Option<bool>,
    pub redis_url: Option<String>,
    pub redis_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SymbolConfigPartial {
    rolling_window: Option<usize>,
    min_periods: Option<usize>,
    quantiles: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SymbolConfigFile {
    default: Option<SymbolConfigPartial>,
    symbols: Option<HashMap<String, SymbolConfigPartial>>,
}

#[derive(Debug, Clone)]
struct SymbolConfig {
    rolling_window: usize,
    min_periods: usize,
    quantiles: Vec<f64>,
}

#[derive(Debug, Clone)]
struct SymbolState {
    config: SymbolConfig,
    values: VecDeque<f64>,
    last_quantiles: Vec<f64>,
    ready: bool,
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
    ) -> Result<String, Box<dyn Error>> {
        let body = serde_json::to_string(payload)?;
        if self.conn.is_none() {
            self.conn = self.client.get_connection().ok();
        }
        let key = format!("{}{}", symbol, self.key_suffix);
        if let Some(conn) = self.conn.as_mut() {
            let res: redis::RedisResult<String> = conn.set(&key, body);
            match res {
                Ok(_) => Ok(key),
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

pub struct RollingRuntime {
    process_cfg_path: String,
    symbols_config_path: String,
    log_factor_thresholds: bool,
    log_redis_write_success: bool,
    reload_sec: u64,
    profile: Option<String>,
    all_symbols: Vec<String>,
    cfg: SymbolConfigFile,
    states: HashMap<String, SymbolState>,
    redis_writer: RedisWriter,
    last_reload: Instant,
    last_mtime: Option<SystemTime>,
}

impl RollingRuntime {
    pub fn new(
        process_cfg_path: &str,
        profile: Option<&str>,
        all_symbols: &[String],
    ) -> Result<Self, Box<dyn Error>> {
        let process_cfg = load_process_config(process_cfg_path)?;
        let symbols_config_path = process_cfg
            .symbols_config
            .clone()
            .unwrap_or_else(|| DEFAULT_SYMBOL_CONFIG.to_string());
        let reload_sec = process_cfg.reload_sec.unwrap_or(DEFAULT_RELOAD_SEC).max(1);
        let log_factor_thresholds = process_cfg.log_factor_thresholds.unwrap_or(false);
        let log_redis_write_success = process_cfg.log_redis_write_success.unwrap_or(false);
        let redis_url = process_cfg.redis_url.clone().unwrap_or_default();
        if redis_url.trim().is_empty() {
            return Err("redis_url is required for rolling runtime".into());
        }

        let redis_key = profile
            .map(redis_key_from_profile)
            .or_else(|| process_cfg.redis_key.clone())
            .unwrap_or_else(|| "_pnlu_factor_thresholds".to_string());

        let mut cfg = load_symbol_config(&symbols_config_path)?;
        validate_overrides(&cfg, all_symbols)?;
        let changed = ensure_symbols_config(&symbols_config_path, &mut cfg, all_symbols)?;
        if changed {
            info!("symbols config expanded {}", symbols_config_path);
        }
        let states = init_states(&cfg, all_symbols)?;

        let redis_writer = RedisWriter::connect(&redis_url, &redis_key).unwrap_or_else(|err| {
            panic!("redis connect failed url={} err={}", redis_url, err)
        });
        info!(
            "rolling runtime ready process_cfg={} symbols={} symbols_config={} reload_sec={} redis_url={} redis_key={}",
            process_cfg_path,
            all_symbols.len(),
            symbols_config_path,
            reload_sec,
            redis_url,
            redis_key
        );

        Ok(Self {
            process_cfg_path: process_cfg_path.to_string(),
            symbols_config_path: symbols_config_path.clone(),
            log_factor_thresholds,
            log_redis_write_success,
            reload_sec,
            profile: profile.map(|v| v.to_string()),
            all_symbols: all_symbols.to_vec(),
            cfg,
            states,
            redis_writer,
            last_reload: Instant::now(),
            last_mtime: file_mtime(&symbols_config_path).ok(),
        })
    }

    pub fn on_factor_row(
        &mut self,
        symbol: &str,
        ts: i64,
        target_ts: i64,
        factor: Option<f64>,
    ) {
        if let Err(err) = self.maybe_reload() {
            warn!("rolling reload failed: {}", err);
        }
        let value = match factor {
            Some(v) => v,
            None => return,
        };
        let state = match self.states.get_mut(symbol) {
            Some(v) => v,
            None => {
                warn!("symbol {} not configured in rolling runtime", symbol);
                return;
            }
        };
        push_value(state, value);
        update_quantiles(state);

        let payload = serde_json::json!({
            "symbol": symbol,
            "ts": ts,
            "target_ts": target_ts,
            "factor": value,
            "quantiles": state.config.quantiles,
            "thresholds": state.last_quantiles,
            "ready": state.ready,
        });
        if self.log_factor_thresholds {
            info!("{}", payload);
        }
        match self.redis_writer.write_json(symbol, &payload) {
            Ok(key) => {
                if self.log_redis_write_success {
                    info!(
                        "redis write ok key={} symbol={} ts={} target_ts={}",
                        key, symbol, ts, target_ts
                    );
                }
            }
            Err(err) => warn!("redis write failed: {}", err),
        }
    }

    fn maybe_reload(&mut self) -> Result<(), Box<dyn Error>> {
        if self.last_reload.elapsed() < Duration::from_secs(self.reload_sec) {
            return Ok(());
        }
        self.last_reload = Instant::now();
        let mtime = match file_mtime(&self.symbols_config_path) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };
        if !self.last_mtime.map(|v| v < mtime).unwrap_or(true) {
            return Ok(());
        }
        self.last_mtime = Some(mtime);
        let mut new_cfg = load_symbol_config(&self.symbols_config_path)?;
        validate_overrides(&new_cfg, &self.all_symbols)?;
        if ensure_symbols_config(&self.symbols_config_path, &mut new_cfg, &self.all_symbols)? {
            self.last_mtime = file_mtime(&self.symbols_config_path).ok();
        }
        let _added = apply_config(
            &mut self.cfg,
            new_cfg,
            &mut self.states,
            &self.all_symbols,
        )?;
        info!(
            "reloaded rolling config {} (process_cfg={} profile={})",
            self.symbols_config_path,
            self.process_cfg_path,
            self.profile.clone().unwrap_or_else(|| "none".to_string())
        );
        Ok(())
    }
}

fn default_partial(cfg: &SymbolConfigFile) -> SymbolConfigPartial {
    cfg.default.clone().unwrap_or(SymbolConfigPartial {
        rolling_window: Some(100000),
        min_periods: Some(10000),
        quantiles: Some(vec![0.1, 0.5, 0.9]),
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

fn load_symbol_config(path: &str) -> Result<SymbolConfigFile, Box<dyn Error>> {
    let raw = fs::read_to_string(path)?;
    let cfg: SymbolConfigFile = serde_json::from_str(&raw)?;
    Ok(cfg)
}

fn load_process_config(path: &str) -> Result<RollingProcessConfigFile, Box<dyn Error>> {
    let mut settings = Config::new();
    settings.merge(File::with_name(path).required(true))?;
    let conf: RollingProcessConfigFile = settings.try_into()?;
    Ok(conf)
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
) -> Result<HashMap<String, SymbolState>, Box<dyn Error>> {
    let mut states = HashMap::new();
    let overrides = cfg.symbols.as_ref();
    for symbol in symbols.iter() {
        let partial = overrides.and_then(|map| map.get(symbol));
        let config = build_symbol_config(cfg, partial);
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
    partial: Option<&SymbolConfigPartial>,
) -> SymbolConfig {
    let default = cfg.default.clone().unwrap_or(SymbolConfigPartial {
        rolling_window: Some(100000),
        min_periods: Some(10000),
        quantiles: Some(vec![0.1, 0.5, 0.9]),
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
    SymbolConfig {
        rolling_window,
        min_periods,
        quantiles,
    }
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
    let mut qs: Vec<f64> = raw.into_iter().filter(|v| v.is_finite()).collect();
    qs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(a.total_cmp(b)));
    qs.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);
    qs
}

fn file_mtime(path: &str) -> Result<SystemTime, Box<dyn Error>> {
    Ok(fs::metadata(path)?.modified()?)
}

fn apply_config(
    current: &mut SymbolConfigFile,
    new_cfg: SymbolConfigFile,
    states: &mut HashMap<String, SymbolState>,
    symbols: &[String],
) -> Result<Vec<String>, Box<dyn Error>> {
    let before: HashSet<String> = states.keys().cloned().collect();
    *current = new_cfg.clone();
    let mut next_states = HashMap::new();
    let overrides = new_cfg.symbols.as_ref();
    for symbol in symbols.iter() {
        let partial = overrides.and_then(|map| map.get(symbol));
        let config = build_symbol_config(&new_cfg, partial);
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

fn redis_key_from_profile(profile: &str) -> String {
    let token = sanitize_profile_token(profile);
    format!("_pnlu_factor_thresholds_{}", token)
}

fn sanitize_profile_token(raw: &str) -> String {
    let t = raw.trim();
    let t = t.trim_matches('/');
    let replaced = t.replace('/', "-");
    if replaced.is_empty() {
        "default".to_string()
    } else {
        replaced
    }
}
