use std::fs::{OpenOptions};
use serde::{Deserialize, Serialize};
use crate::gconf::{ENGIN_CONF};
use csv::{Writer,WriterBuilder};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordDumpItem {
    pub create_ts:i64,
    pub update_ts:i64,
    pub client_order_id:String,
    pub symbol:String,
    pub ttype:String,
    pub sid:i32,
    pub side:String,
    pub price:f64,
    pub amount_init:f64,
    pub amount_update:f64,
    pub status:String,
    pub inpos:f64,
    pub tlen:f64,
    pub from_key:String,
    pub bid1:f64,
    pub ask1:f64
}

#[derive(Debug, Clone)]
pub struct TSRecordItem {
    pub create_ts:i64,
    pub symbol:String,
    pub sid:i32,
    pub pos:f64,
    pub open:f64,
}


pub fn write_to_csv(rd:&RecordDumpItem) {

   
    let file_name = ENGIN_CONF.dump_path.to_string()+"/orders.csv";
    
   // let mut csv_writer = Writer::from_path(file_name).expect("Failed to create CSV writer");



    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_name)
        .unwrap();

    let mut csv_writer = WriterBuilder::new().from_writer(file);

    //create_ts:pitem.create_ts,update_ts:update_ts_ms,client_order_id:pitem.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), sid:*sid, side:side.to_string(), price:pitem.price, amount_init:pitem.amount_init, amount_update:pitem.amount, status:status
    let _ = csv_writer.write_record(&[rd.client_order_id.clone(), rd.create_ts.to_string(),rd.update_ts.to_string(),rd.client_order_id.clone(),rd.symbol.to_string(),rd.ttype.to_string(),rd.sid.to_string(),rd.side.to_string(),rd.price.to_string(),rd.amount_init.to_string(),rd.amount_update.to_string(),rd.status.to_string(), rd.inpos.to_string(), rd.tlen.to_string(), rd.from_key.to_string(), rd.bid1.to_string(), rd.ask1.to_string()]);

    let _ = csv_writer.flush();
    
}


pub fn write_to_csv_ts(rd:&TSRecordItem) {

   
    let file_name = ENGIN_CONF.dump_path.to_string()+"/nps.csv";
    
   // let mut csv_writer = Writer::from_path(file_name).expect("Failed to create CSV writer");



    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_name)
        .unwrap();

    let mut csv_writer = WriterBuilder::new().from_writer(file);

    //create_ts:pitem.create_ts,update_ts:update_ts_ms,client_order_id:pitem.client_order_id.to_string(),symbol:tinfo.symbol.to_string(), sid:*sid, side:side.to_string(), price:pitem.price, amount_init:pitem.amount_init, amount_update:pitem.amount, status:status
    let _ = csv_writer.write_record(&[rd.create_ts.to_string(), rd.symbol.to_string(), rd.sid.to_string(), rd.pos.to_string(), rd.open.to_string()]);

    let _ = csv_writer.flush();
    
}