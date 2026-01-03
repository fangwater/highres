#![allow(clippy::unnecessary_wraps)]
//! Get all trading pairs of a cryptocurrency exchange.
//!
//! ## Example
//!
//! ```
//! use crypto_markets::fetch_markets;
//! use crypto_market_type::MarketType;
//!
//! let markets = fetch_markets("binance", MarketType::Spot).unwrap();
//! println!("{}", serde_json::to_string_pretty(&markets).unwrap())
//! ```

mod error;
mod exchanges;
mod market;


// use crypto_market_type::MarketType;
use market_type::MarketType;



pub use error::Error;
pub use market::{Fees, Market, Precision, QuantityLimit};

use error::Result;

use std::{fs, io};
use std::path::Path;
use serde_json;


// 构建缓存文件名
fn cache_filename(exchange: &str, market_type: &MarketType) -> String {
    // println!(market_type)
    format!("{}_{}_cache.json", exchange, market_type.as_str())
}

// 检查缓存是否有效
fn is_cache_valid(exchange: &str, market_type: &MarketType) -> bool {
    Path::new(&cache_filename(exchange, market_type)).exists()
}

// 从缓存中读取市场数据
fn read_cache(exchange: &str, market_type: &MarketType) -> io::Result<Vec<Market>> {
    let cache_content = fs::read_to_string(cache_filename(exchange, market_type))?;
    let markets = serde_json::from_str(&cache_content)?;
    Ok(markets)
}

// 将市场数据写入缓存
fn write_cache(exchange: &str, market_type: &MarketType, markets: &[Market]) -> io::Result<()> {
    let json = serde_json::to_string(markets)?;
    fs::write(cache_filename(exchange, market_type), json)?;
    Ok(())
}

/// Fetch trading symbols.
pub fn fetch_symbols(exchange: &str, market_type: MarketType) -> Result<Vec<String>> {
    match exchange {
        "binance" => exchanges::binance::fetch_symbols(market_type),
        "bitfinex" => exchanges::bitfinex::fetch_symbols(market_type),
        "bitget" => exchanges::bitget::fetch_symbols(market_type),
        "bithumb" => exchanges::bithumb::fetch_symbols(market_type),
        // "bitmex" => exchanges::bitmex::fetch_symbols(market_type),
        "bitstamp" => exchanges::bitstamp::fetch_symbols(market_type),
        "bitz" => exchanges::bitz::fetch_symbols(market_type),
        "bybit" => exchanges::bybit::fetch_symbols(market_type),
        "coinbase_pro" => exchanges::coinbase_pro::fetch_symbols(market_type),
        "deribit" => exchanges::deribit::fetch_symbols(market_type),
        "dydx" => exchanges::dydx::fetch_symbols(market_type),
        "ftx" => exchanges::ftx::fetch_symbols(market_type),
        "gate" => exchanges::gate::fetch_symbols(market_type),
        "huobi" => exchanges::huobi::fetch_symbols(market_type),
        "kraken" => exchanges::kraken::fetch_symbols(market_type),
        "kucoin" => exchanges::kucoin::fetch_symbols(market_type),
        "mexc" => exchanges::mexc::fetch_symbols(market_type),
        "okx" => exchanges::okx::fetch_symbols(market_type),
        "zb" => exchanges::zb::fetch_symbols(market_type),
        "zbg" => exchanges::zbg::fetch_symbols(market_type),
        _ => panic!("Unsupported exchange {exchange}"),
    }
}

/// Fetch trading markets of a cryptocurrency exchange.
///
/// # Arguments
///
/// * `exchange` - The exchange name
/// * `market_type` - The market type
///
/// # Example
///
/// ```
/// use crypto_markets::fetch_markets;
/// use crypto_market_type::MarketType;
/// let markets = fetch_markets("binance", MarketType::Spot).unwrap();
/// assert!(!markets.is_empty());
/// println!("{}", serde_json::to_string_pretty(&markets).unwrap())
/// ```
pub fn fetch_markets(exchange: &str, market_type: MarketType) -> Result<Vec<Market>> {
    // 首先检查缓存是否有效
    if is_cache_valid(exchange, &market_type) {
        // 如果有效，就读取缓存
        if let Ok(markets) = read_cache(exchange, &market_type) {
            return Ok(markets);
        }
        panic!("fetch_markets read cache err");
    }
    
    let result = match exchange {
        "binance" => exchanges::binance::fetch_markets(market_type),
        "bitfinex" => exchanges::bitfinex::fetch_markets(market_type),
        "bitget" => exchanges::bitget::fetch_markets(market_type),
        "bithumb" => exchanges::bithumb::fetch_markets(market_type),
        // "bitmex" => exchanges::bitmex::fetch_markets(market_type),
        "bitstamp" => exchanges::bitstamp::fetch_markets(market_type),
        "bitz" => exchanges::bitz::fetch_markets(market_type),
        "bybit" => exchanges::bybit::fetch_markets(market_type),
        "coinbase_pro" => exchanges::coinbase_pro::fetch_markets(market_type),
        "deribit" => exchanges::deribit::fetch_markets(market_type),
        "dydx" => exchanges::dydx::fetch_markets(market_type),
        "ftx" => exchanges::ftx::fetch_markets(market_type),
        "gate" => exchanges::gate::fetch_markets(market_type),
        "huobi" => exchanges::huobi::fetch_markets(market_type),
        "kraken" => exchanges::kraken::fetch_markets(market_type),
        "kucoin" => exchanges::kucoin::fetch_markets(market_type),
        "mexc" => exchanges::mexc::fetch_markets(market_type),
        "okx" => exchanges::okx::fetch_markets(market_type),
        "zb" => exchanges::zb::fetch_markets(market_type),
        "zbg" => exchanges::zbg::fetch_markets(market_type),
        _ => panic!("Unsupported exchange {exchange}"),
    };
    
    match result {
        Ok(markets) => {
            if let Err(e) = write_cache(exchange, &market_type, markets.as_slice()) {
                eprintln!("fetch_markets write cache err: {}", e);
            }
            
            Ok(markets)
        },
        Err(e) => {
            panic!("fetch_markets: {}", e);
        },
    }
}
