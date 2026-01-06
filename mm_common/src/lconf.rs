use config::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct NodeConf {
    #[allow(dead_code)]
    pub exchange: String,
    #[allow(dead_code)]
    pub etype: String,
    #[allow(dead_code)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Deserialize)] 
pub struct MarketNodeConf {
    market: Vec<NodeConf>,
} 

pub fn load_config(config_file: &str) -> Result<Vec<NodeConf>, ConfigError> {
    let mut settings = Config::new();
    settings.merge(File::with_name(config_file).required(true))?;
    let marketnodeconf: MarketNodeConf = settings.try_into()?;

    Ok(marketnodeconf.market) 
}