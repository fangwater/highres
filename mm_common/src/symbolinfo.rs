
pub fn symbol_std2plat(symbolstr: &str, exchange : &str, etype: &str) -> String {
    match (exchange, etype) {
        ("binance", "spot") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("binance", "swap") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("okx", "spot") => {
            let nsymbolstr = symbolstr.replace("_", "-").to_uppercase();
            nsymbolstr.to_string()
        },
        ("okx", "swap") => {
            let nsymbolstr = symbolstr.replace("_usdt", "-usdt-swap").to_uppercase();
            nsymbolstr.to_string()
        },
        ("bybit", "swap") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("bybit", "spot") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("bitget", "spot") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("bitget", "swap") => {
            let nsymbolstr = symbolstr.replace("_", "").to_uppercase();
            nsymbolstr.to_string()
        },
        ("kucoin", "spot") => {
            let nsymbolstr = symbolstr.replace("_", "-").to_uppercase();
            nsymbolstr.to_string()
        },
        ("kucoin", "swap") => {
            let nsymbolstr = symbolstr.replace("_usdt", "USDTM").to_uppercase();
            nsymbolstr.to_string()
        },
        ("gate", "spot") => {
            let nsymbolstr = symbolstr.to_uppercase();
            nsymbolstr.to_string()
        },
        ("gate", "swap") => {
            let nsymbolstr = symbolstr.to_uppercase();
            nsymbolstr.to_string()
        },
        _ => {
            panic!("Unknown exchange {exchange} {etype}")
        }
    }
}

pub fn symbol_plat2std(symbolstr: &str, exchange : &str, etype: &str) -> String {
    match (exchange, etype) {
        ("okx", "spot") => {
            let nsymbolstr = symbolstr.replace("-", "_").to_lowercase();
            nsymbolstr.to_string()
        },
        ("okx", "swap") => {
            let p:Vec<String> =  symbolstr.split("-USDT-SWAP").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";

            nsymbolstr.to_string()
        },
        ("binance", "swap") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("binance", "spot") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("bybit", "swap") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("bybit", "spot") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("bitget", "spot") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("bitget", "swap") => {
            //let p:Vec<String> =  symbolstr.split("USDT_UMCBL").map(|s| s.to_string()).collect();
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("kucoin", "spot") => {
            let nsymbolstr = symbolstr.replace("-", "_").to_lowercase();
            nsymbolstr.to_string()
        },
        ("kucoin", "swap") => {
            let p:Vec<String> =  symbolstr.split("USDT").map(|s| s.to_string()).collect();
            let nsymbolstr = p[0].to_string().to_lowercase()+"_usdt";
            
            nsymbolstr.to_string()
        },
        ("gate", "spot") => {
            let nsymbolstr = symbolstr.to_lowercase();
            nsymbolstr.to_string()
        },
        ("gate", "swap") => {
            let nsymbolstr = symbolstr.to_lowercase();
            nsymbolstr.to_string()
        },
        _ => {
            panic!("Unknown exchange {exchange}")
        }
    }
}

pub fn get_token_quote_from_symbol(symbol: &str)->(String, String) {
    let s: Vec<String> = symbol.split("_").map(|s| s.to_string()).collect();

    (s[0].to_string(), s[1].to_string())
}

