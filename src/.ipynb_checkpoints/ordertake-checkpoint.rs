use log::{info};
use crate::gconf::{ENGIN_CONF};
use crate::trade::{TradeInfo, DepthInfo, MakeDecision};
use crate::record::{write_to_csv, RecordDumpItem};
use crate::symbolinfo::get_market;

pub fn add_taking(tinfo:&mut TradeInfo, tds:&MakeDecision) {
    let sid:i32 = tds.sid;
    let dinfo:&mut DepthInfo = tinfo.depths.get_mut(&sid).unwrap();
    let mut deal_amount:f64 = 0.;
    let amount_f64: f64 = tds.amount;
    let mut total_amountprice:f64 = 0.;
    let mut total_deal_avg_price:f64 = 0.;
    let mut total_deal_token:f64 = 0.;
    
    let e = &ENGIN_CONF.sids[&sid];
    let market = get_market().get((e["exchange"].to_string()+":"+&e["etype"]+":"+&tinfo.symbol_std).as_str()).unwrap();
    
    let mut contract_value = 1.;
    if e["etype"] == "swap"  && e["exchange"] == "okx" {
        contract_value = market.contract_value.unwrap();
        println!("contract_value={}",contract_value);
    }

  
    if tds.side == "buy".to_string() {
        //check ask-spreads
        for (price, amount) in dinfo.asks.iter() {
            //info!("t asks {}: {}", price, amount);
            let left_amount = amount_f64 - deal_amount;
            if amount > &left_amount {
                //finished
                total_amountprice+=left_amount * price.into_inner();
                deal_amount+=left_amount;
                let p = tinfo.pos.get_mut(&sid).unwrap();
                *p+=left_amount;
                tinfo.open+=left_amount*contract_value;
                total_deal_avg_price = total_amountprice / deal_amount;
                total_deal_token = deal_amount;
                
                info!("finish taker-buy, deal_amount={} total_deal_avg_price={}", deal_amount, total_deal_avg_price);
                break
            }
            else {
                let qc = amount * price.into_inner();
                total_amountprice+=qc;
                deal_amount+=amount;
                let p = tinfo.pos.get_mut(&sid).unwrap();
                *p+=amount;
                tinfo.open+=amount*contract_value;
            }
        }
        if total_deal_token == 0. {
            //unfinished
            let avg_price = total_amountprice / deal_amount;
            total_deal_avg_price = avg_price;
            info!("finish taker-buy(nol2offered), deal_amount={} total_deal_avg_price={}", deal_amount, total_deal_avg_price);
        }
        
        if total_deal_token >= amount_f64 {
            let status = "filled".to_string();
            let update_ts_ms = tds.create_ts * 1000;
            let rd:RecordDumpItem = RecordDumpItem{
                create_ts:update_ts_ms,
                update_ts:update_ts_ms,
                client_order_id:tds.client_order_id.to_string(),
                symbol:tinfo.symbol.to_string(), 
                ttype:"taker".to_string(), 
                sid:sid, 
                side:tds.side.to_string(), 
                price:total_deal_avg_price,
                amount_init:tds.amount*contract_value, 
                amount_update:(deal_amount)*contract_value, 
                status:status,
                inpos:0.,
                tlen:0.,
                from_key:tds.from_key.to_string(),
                bid1:dinfo.bid1,
                ask1:dinfo.ask1
            };
            write_to_csv(&rd);
        }
        else {
            let status = "tcanceled".to_string();
            let update_ts_ms = tds.create_ts * 1000;
            let rd:RecordDumpItem = RecordDumpItem{
                create_ts:update_ts_ms,
                update_ts:update_ts_ms,
                client_order_id:tds.client_order_id.to_string(),
                symbol:tinfo.symbol.to_string(), 
                ttype:"taker".to_string(), 
                sid:sid, 
                side:tds.side.to_string(), 
                price:total_deal_avg_price,
                amount_init:tds.amount*contract_value, 
                amount_update:(deal_amount)*contract_value, 
                status:status,
                inpos:0.,
                tlen:0.,
                from_key:tds.from_key.to_string(),
                bid1:dinfo.bid1,
                ask1:dinfo.ask1
            };
            write_to_csv(&rd);
        }
        
    
    } else if tds.side == "sell".to_string() {

        for (price, amount) in dinfo.bids.iter().rev() {
            //info!("t asks {}: {}", price, amount);
            let left_amount = amount_f64 - deal_amount;
            if amount > &left_amount {
                //finished
                total_amountprice+=left_amount * price.into_inner();
                deal_amount+=left_amount;
                let p = tinfo.pos.get_mut(&sid).unwrap();
                *p-=left_amount;
                tinfo.open-=left_amount*contract_value;
                total_deal_avg_price = total_amountprice / deal_amount;
                total_deal_token = deal_amount;
                
                info!("finish taker-sell, deal_amount={} total_deal_avg_price={} amount_f64={}", deal_amount, total_deal_avg_price, amount_f64);
                break
            }
            else {
                let qc = amount * price.into_inner();
                total_amountprice+=qc;
                deal_amount+=amount;
                let p = tinfo.pos.get_mut(&sid).unwrap();
                *p-=amount;
                tinfo.open-=amount*contract_value;
            }
        }
        if total_deal_token == 0. {
            //unfinished
            let avg_price = total_amountprice / deal_amount;
            total_deal_avg_price = avg_price;
            info!("finish taker-sell(nol2offered), deal_amount={} total_deal_avg_price={} amount_f64={}", deal_amount, total_deal_avg_price, amount_f64);
        }
        
        if total_deal_token >= amount_f64 {
            let status = "filled".to_string();
            let update_ts_ms = tds.create_ts * 1000;
            let rd:RecordDumpItem = RecordDumpItem{
                create_ts:update_ts_ms,
                update_ts:update_ts_ms,
                client_order_id:tds.client_order_id.to_string(),
                symbol:tinfo.symbol.to_string(), 
                ttype:"taker".to_string(), 
                sid:sid, 
                side:tds.side.to_string(), 
                price:total_deal_avg_price,
                amount_init:tds.amount*contract_value, 
                amount_update:(deal_amount)*contract_value, 
                status:status,
                inpos:0.,
                tlen:0.,
                from_key:tds.from_key.to_string(),
                bid1:dinfo.bid1,
                ask1:dinfo.ask1
            };
            write_to_csv(&rd);
        }
        else {
            let status = "tcanceled".to_string();
            let update_ts_ms = tds.create_ts * 1000;
            let rd:RecordDumpItem = RecordDumpItem{
                create_ts:update_ts_ms,
                update_ts:update_ts_ms,
                client_order_id:tds.client_order_id.to_string(),
                symbol:tinfo.symbol.to_string(), 
                ttype:"taker".to_string(), 
                sid:sid, 
                side:tds.side.to_string(), 
                price:total_deal_avg_price,
                amount_init:tds.amount*contract_value, 
                amount_update:(deal_amount)*contract_value, 
                status:status,
                inpos:0.,
                tlen:0.,
                from_key:tds.from_key.to_string(),
                bid1:dinfo.bid1,
                ask1:dinfo.ask1
            };
            write_to_csv(&rd);
        }
        
        
    } else {
        panic!("side={}", tds.side);
    }
}
