use config::*;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct EnginConf {
    #[allow(dead_code)]
    pub symbols: Vec<String>,
    pub sids: HashMap<i32, HashMap<String, String>>,
    pub vsids: Vec<i32>,
    pub stg: String,
    pub dump_path: String,
    pub cancel_delay_ms: i64,
    pub is_spending_open_dump: bool,
    pub is_spending_tick_dump: bool,
    pub tick_dump_modts: i64,
    pub is_target_sid_open_keep: bool,
}

#[derive(Debug, Deserialize)]
pub struct PairmmConf {
    pub amountu: f64,
    pub max_open_order_keep_s: i64,
    pub max_close_order_keep_s: i64,
    pub tickpath: String,
    #[allow(dead_code)]
    pub sample_path: String,
    #[allow(dead_code)]
    pub close_df: f64,
    pub close_ts: f64,
    pub open_ranges: Vec<f64>,
    pub close_rb: f64,
    pub sample_step: i64,
    pub is_start_close_open: bool,
    pub tlenu: f64,
    pub fdf: f64,
    pub is_opne_price_gap_ratio: bool,
    pub opne_price_gap_ratio: f64,
    pub opne_sid: i32,
    pub max_pos_u: f64,
    #[serde(default = "default_open_snapshot_warmup_s")]
    pub open_snapshot_warmup_s: i64,
}

lazy_static! {
    pub static ref ENGIN_CONF: EnginConf = {
        let config_file = highres_config_path();
        load_config_aenginconf(&config_file).unwrap()
    };
    pub static ref PAIRMM_CONF: PairmmConf = {
        let config_file = highres_config_path();
        load_config_pairmmconf(&config_file).unwrap()
    };
}

fn highres_config_path() -> String {
    std::env::var("HIGHRES_CONFIG_PATH")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "highres.toml".to_string())
}

fn default_open_snapshot_warmup_s() -> i64 {
    30 * 60
}

pub fn load_config_aenginconf(config_file: &str) -> Result<EnginConf, ConfigError> {
    let mut settings = Config::new();
    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("engin").unwrap();
    let engin_conf: EnginConf = engin.try_into().unwrap();
    Ok(engin_conf)
}

pub fn load_config_pairmmconf(config_file: &str) -> Result<PairmmConf, ConfigError> {
    let mut settings = Config::new();
    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("pairmm").unwrap();
    let engin_conf: PairmmConf = engin.try_into().unwrap();
    Ok(engin_conf)
}
