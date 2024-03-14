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
    pub is_spending_tick_dump:bool
}

#[derive(Debug, Deserialize)]
pub struct ArbmmConf {
    pub amountu:f64,
    pub max_order_keep_s:i32
}

#[derive(Debug, Deserialize)]
pub struct Arbmm2Conf {
    pub amountu:f64,
    pub max_order_keep_s:i32
}

#[derive(Debug, Deserialize)]
pub struct ArbmtSamplingConf {
    pub amountu:f64,
    pub ranges:Vec<f64>,
    pub max_order_keep_s:i64,
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
    pub static ref ARBMM2_CONF: Arbmm2Conf = {
        let econf:Arbmm2Conf = load_config_arbmm2conf("highres.toml").unwrap();
        econf
    };
    pub static ref ARBMTSAMPLING_CONF: ArbmtSamplingConf = {
        let econf:ArbmtSamplingConf = load_config_arbmmsamplingconf("highres.toml").unwrap();
        econf
    };
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

