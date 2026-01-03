use std::collections::HashMap;
use std::collections::BTreeMap;
use ordered_float::OrderedFloat;



#[derive(Clone, Debug)]
pub struct DepthInfo {
    pub sid:i32,
    pub exchange:String,
    pub etype:String,
    pub is_snaping:bool,
    pub is_finish_snap:bool,
    pub bids:BTreeMap<OrderedFloat<f64>, f64>,
    pub asks:BTreeMap<OrderedFloat<f64>, f64>,
    pub bid1:f64,
    pub ask1:f64
    
}

#[derive(Clone, Debug)]
pub struct TradeInfo {
    pub symbol:String,
    pub symbol_std:String,
    pub depths:HashMap<i32,DepthInfo>,
    pub pos:HashMap<i32,f64>,
    pub open:f64
}


impl TradeInfo {
    pub fn new(symbol:String, sids:&HashMap<i32, HashMap<String, String>>) -> TradeInfo {
        let mut depths:HashMap<i32,DepthInfo> = HashMap::new();
        let mut pos:HashMap<i32,f64> = HashMap::new();
        
        for (sid, sinfo) in sids.iter() {
            let dinfo = DepthInfo {sid:*sid, exchange:sinfo["exchange"].to_string(), etype:sinfo["etype"].to_string(), is_snaping:false, is_finish_snap:false, bids:BTreeMap::new(), asks:BTreeMap::new(), bid1:0., ask1:0.};
            depths.insert(*sid, dinfo);
            pos.insert(*sid, 0.);
        }
        let parts: Vec<&str> = symbol.split("USDT").collect();
        let symbol_std = parts[0].to_string().to_lowercase()+"_usdt";

        
        
        TradeInfo {
            symbol:symbol, symbol_std:symbol_std, depths:depths, pos:pos, open:0.
        }
    }
}

#[derive(Clone, Debug)]
pub struct MakeDecision {
    pub create_ts:i64,
    pub client_order_id:String,
    pub side:String,
    pub sid:i32,
    pub ttype:String,
    pub price:f64,
    pub amount:f64,
    pub max_order_keep_s:i32,
    pub from_key:String,
    pub dup_key:String,
    pub target_sid:i32
}


#[derive(Clone, Debug)]
pub struct CancelDecision {
    pub sid:i32,
    pub side:String, // bid,ask
    pub price:f64,
    pub client_order_id:String,
    pub delayup_ms:i64
}
