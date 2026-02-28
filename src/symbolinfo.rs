use crate::gconf::ENGIN_CONF;
use market_type::MarketType;
use markets::fetch_markets;
use mm_common::symbolinfo::symbol_plat2std;
use once_cell::sync::OnceCell;
use std::collections::HashMap;

use markets::Market;
use markets::QuantityLimit;

pub fn get_market() -> &'static HashMap<String, Market> {
    static INSTANCE: OnceCell<HashMap<String, Market>> = OnceCell::new();
    INSTANCE.get_or_init(|| {
        let mut exchange_info: HashMap<String, Market> = HashMap::new();

        for (_sid, sinfo) in ENGIN_CONF.sids.iter() {
            let mt;
            match sinfo["etype"].as_str() {
                "spot" => mt = MarketType::Spot,
                "swap" => mt = MarketType::LinearSwap,
                _ => {
                    panic!("etype exchange info");
                }
            }
            println!("{},{}", sinfo["exchange"], mt);
            if let Ok(mut markets) = fetch_markets(&sinfo["exchange"], mt) {
                for market in markets.iter_mut() {
                    let symbol_std = symbol_plat2std(
                        &market.symbol,
                        sinfo["exchange"].as_str(),
                        sinfo["etype"].as_str(),
                    );
                    let exchange_info_key =
                        sinfo["exchange"].to_string() + ":" + &sinfo["etype"] + ":" + &symbol_std;
                    let mut market_rc = market.clone();

                    if (&sinfo["exchange"] == "gate") && (sinfo["etype"] == "swap") {
                        // 获取 quantity_limit 的 min 字段
                        match market_rc.quantity_limit {
                            Some(QuantityLimit { min, .. }) => match min {
                                Some(min_value) => {
                                    market_rc.precision.lot_size = min_value;
                                }
                                None => {
                                    println!("Min value is not set.");
                                }
                            },
                            None => {
                                println!("Quantity limit is not set.");
                            }
                        }
                    } else if (&sinfo["exchange"] == "binance") && (sinfo["etype"] == "swap") {
                        if market_rc.info.contains_key("filters") {
                            if let Some(filters) = market_rc.info["filters"].as_array() {
                                for filter in filters.iter() {
                                    if filter["filterType"] == "PRICE_FILTER" {
                                        let tick_size = filter["tickSize"]
                                            .as_str()
                                            .unwrap()
                                            .parse::<f64>()
                                            .unwrap();
                                        market_rc.precision.tick_size = tick_size;
                                    }
                                }
                            }
                        }
                    }

                    exchange_info.insert(exchange_info_key, market_rc);
                }
            } else {
                panic!("fetch exchangeinfo err");
            }
        }
        //println!("ei={:?}", exchange_info);
        exchange_info
    })
}
