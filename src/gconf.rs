use config::*;
use serde::Deserialize;
use lazy_static::lazy_static;
use std::collections::HashMap;


#[derive(Debug, Deserialize)]
pub struct EnginConf {
    pub merged_market_path:String,
    pub tick_path:String,
    pub start_date:String,
    pub end_date:String,
    pub symbols:Vec<String>,
    pub sids:HashMap<i32, HashMap<String, String>>,
    pub vsids:Vec<i32>,
    pub stg:String,
    pub dump_path:String,
    pub start_ts:i64,
    pub end_ts:i64,
    pub cancel_delay_ms:i64,
    pub is_spending_open_dump:bool,
    pub is_spending_tick_dump:bool,
    pub tick_dump_modts:i64,
    pub is_target_sid_open_keep:bool,
    
}

#[derive(Debug, Deserialize)]
pub struct ArbmmConf {
    pub amountu:f64,
    pub max_order_keep_s:i32
}

#[derive(Debug, Deserialize)]
pub struct PairmmConf {
    pub amountu:f64,
    pub max_open_order_keep_s:i64,
    pub max_close_order_keep_s:i64,
    pub tickpath:String,
    pub sample_path:String,
    pub close_df:f64,
    pub close_ts:f64,
    pub open_ranges:Vec<f64>,
    pub close_rb:f64,
    pub sample_step:i64,
    pub is_start_close_open:bool,
    pub tlenu:f64,
    pub fdf:f64,
    pub is_opne_price_gap_ratio:bool,
    pub opne_price_gap_ratio:f64,
    pub opne_sid:i32,
    pub max_pos_u:f64,
    #[serde(default = "default_open_snapshot_warmup_s")]
    pub open_snapshot_warmup_s:i64,
    
}

#[derive(Debug, Deserialize)]
pub struct Arbmm2Conf {
    pub amountu:f64,
    pub max_order_keep_s:i32,
    pub fees:HashMap<i32, HashMap<String, f64>>,

}

#[derive(Debug, Deserialize)]
pub struct ArbmtSamplingConf {
    pub amountu:f64,
    pub ranges:Vec<f64>,
    pub max_order_keep_s:i64,
}

#[derive(Debug, Deserialize)]
pub struct NmtConf {
    pub amountu:f64,
    pub max_order_keep_s:i32,
    pub max_pos:f64,
    pub fees:HashMap<i32, HashMap<String, f64>>,
    pub thr_add_open:f64,
    pub thr_add_close:f64,
    pub gconf_open:String,
    pub gconf_close:String,
    pub stat_path:String,
    pub profit_range:f64
}

lazy_static! {
    pub static ref ENGIN_CONF: EnginConf = {
        let econf:EnginConf = load_config_aenginconf("highres.toml").unwrap();
        econf
    };
    pub static ref ARBMM_CONF: ArbmmConf = {
        let econf:ArbmmConf = load_config_arbmmconf("highres.toml").unwrap();
        econf
    };
    pub static ref PAIRMM_CONF: PairmmConf = {
        let econf:PairmmConf = load_config_pairmmconf("highres.toml").unwrap();
        econf
    };
    pub static ref ARBMM2_CONF: Arbmm2Conf = {
        let econf:Arbmm2Conf = load_config_arbmm2conf("highres.toml").unwrap();
        econf
    };
    pub static ref ARBMTSAMPLING_CONF: ArbmtSamplingConf = {
        let econf:ArbmtSamplingConf = load_config_arbmmsamplingconf("highres.toml").unwrap();
        econf
    };
    pub static ref NMT_CONF: NmtConf = {
        let econf:NmtConf = load_config_nmtconf("highres.toml").unwrap();
        econf
    };
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

pub fn load_config_arbmmconf(config_file: &str) -> Result<ArbmmConf, ConfigError> {
    let mut settings = Config::new();

    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("arbmm").unwrap();
    let engin_conf: ArbmmConf = engin.try_into().unwrap();
    Ok(engin_conf)
}

pub fn load_config_pairmmconf(config_file: &str) -> Result<PairmmConf, ConfigError> {
    let mut settings = Config::new();

    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("pairmm").unwrap();
    let engin_conf: PairmmConf = engin.try_into().unwrap();
    Ok(engin_conf)
}


pub fn load_config_arbmm2conf(config_file: &str) -> Result<Arbmm2Conf, ConfigError> {
    let mut settings = Config::new();

    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("arbmm2").unwrap();
    let engin_conf: Arbmm2Conf = engin.try_into().unwrap();
    Ok(engin_conf)
}


pub fn load_config_arbmmsamplingconf(config_file: &str) -> Result<ArbmtSamplingConf, ConfigError> {
    let mut settings = Config::new();

    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("arbmt_sampling").unwrap();
    let engin_conf: ArbmtSamplingConf = engin.try_into().unwrap();
    Ok(engin_conf)
}

pub fn load_config_nmtconf(config_file: &str) -> Result<NmtConf, ConfigError> {
    let mut settings = Config::new();

    settings.merge(File::with_name(config_file).required(true))?;
    let engin: Value = settings.get("nmt").unwrap();
    let engin_conf: NmtConf = engin.try_into().unwrap();
    Ok(engin_conf)
}

