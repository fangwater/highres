use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone)]
pub struct FactorConfig {
    pub period_s: i64,
    pub rolling_window: usize,
    pub min_periods: usize,
    pub shift: usize,
    pub max_keep_periods: usize,
}

#[derive(Debug, Clone)]
pub struct OrderItem {
    pub client_order_id: String,
    pub create_ts: i64,
    pub update_ts: i64,
    pub sid: i32,
    pub side: String,
    pub price: f64,
    pub amount_update: f64,
    pub tlen: f64,
    pub from_key: String,
}

impl OrderItem {
    pub fn ts_ms(&self) -> i64 {
        if self.update_ts > 0 {
            self.update_ts
        } else {
            self.create_ts
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutputRow {
    pub ts: i64,
    pub target_ts: i64,
    pub pnlu_sum: Option<f64>,
    pub factor: Option<f64>,
}

#[derive(Clone)]
struct OpenAgg {
    cts_sec: i64,
    uts_sec: i64,
    price: f64,
    side: String,
    tlen_notional: f64,
    famount_sum: f64,
}

#[derive(Clone)]
struct CloseAgg {
    fts_sec: i64,
    close_count: usize,
    camount_sum: f64,
    pa_sum: f64,
}

pub struct FactorState {
    period_s: i64,
    shift_seconds: i64,
    rolling_window: usize,
    min_periods: usize,
    max_keep_seconds: i64,

    open_by_fkey: HashMap<String, OpenAgg>,
    close_by_fkey: HashMap<String, CloseAgg>,
    open_buckets: HashMap<i64, HashSet<String>>,
    close_buckets: HashMap<i64, HashSet<String>>,
    open_bucket_order: VecDeque<i64>,
    close_bucket_order: VecDeque<i64>,
    open_bucket_seen: HashSet<i64>,
    close_bucket_seen: HashSet<i64>,

    roll_values: VecDeque<Option<f64>>,
    roll_sum: f64,
    roll_count: usize,
    last_factor: Option<f64>,
    last_tp: Option<i64>,
}

impl FactorState {
    pub fn new(conf: &FactorConfig) -> Self {
        let shift_seconds = conf.shift as i64 * conf.period_s;
        let max_keep_seconds = conf.max_keep_periods as i64 * conf.period_s;
        Self {
            period_s: conf.period_s,
            shift_seconds,
            rolling_window: conf.rolling_window,
            min_periods: conf.min_periods,
            max_keep_seconds,
            open_by_fkey: HashMap::new(),
            close_by_fkey: HashMap::new(),
            open_buckets: HashMap::new(),
            close_buckets: HashMap::new(),
            open_bucket_order: VecDeque::new(),
            close_bucket_order: VecDeque::new(),
            open_bucket_seen: HashSet::new(),
            close_bucket_seen: HashSet::new(),
            roll_values: VecDeque::new(),
            roll_sum: 0.0,
            roll_count: 0,
            last_factor: None,
            last_tp: None,
        }
    }

    pub fn pending_counts(&self) -> (usize, usize) {
        (self.open_by_fkey.len(), self.close_by_fkey.len())
    }

    pub fn process_order(&mut self, item: &OrderItem, emit: bool) -> Vec<OutputRow> {
        let oid = item.client_order_id.as_str();
        let cts_sec = (item.create_ts / 1000) as i64;
        let uts_sec = (item.ts_ms() / 1000) as i64;
        let sid = item.sid;
        let side = item.side.to_ascii_lowercase();
        let price = item.price;
        let famount = item.amount_update;
        let tlen = item.tlen;
        let raw_fkey = item.from_key.as_str();

        let current_tp = (uts_sec / self.period_s) * self.period_s;
        let mut outputs = Vec::new();

        if let Some(last_tp) = self.last_tp {
            if current_tp > last_tp + self.period_s {
                self.last_tp = Some(current_tp - self.period_s);
            }
        } else {
            self.last_tp = Some(current_tp - self.period_s);
        }
        while let Some(last_tp) = self.last_tp {
            if current_tp <= last_tp {
                break;
            }
            let next_tp = last_tp + self.period_s;
            self.last_tp = Some(next_tp);
            self.expire_old(next_tp);
            let target_ts = next_tp - self.shift_seconds;
            let pnlu_value = self.finalize_bucket(target_ts);
            let mean = self.update_rolling(pnlu_value);
            let factor = if let Some(v) = mean {
                self.last_factor = Some(v);
                Some(v)
            } else {
                self.last_factor
            };
            if emit {
                outputs.push(OutputRow {
                    ts: next_tp,
                    target_ts,
                    pnlu_sum: pnlu_value,
                    factor,
                });
            }
        }

        if oid.starts_with('o') && sid == 0 {
            let open_tp = (cts_sec / self.period_s) * self.period_s;
            if let Some(rec) = self.open_by_fkey.get_mut(raw_fkey) {
                rec.uts_sec = uts_sec;
                rec.price = price;
                rec.side = side;
                rec.tlen_notional = tlen * price;
                rec.famount_sum += famount;
            } else {
                self.open_by_fkey.insert(
                    raw_fkey.to_string(),
                    OpenAgg {
                        cts_sec,
                        uts_sec,
                        price,
                        side,
                        tlen_notional: tlen * price,
                        famount_sum: famount,
                    },
                );
                self.add_open_bucket(open_tp, raw_fkey);
            }
        } else if oid.starts_with('c') {
            let close_fkey = parse_close_fkey(raw_fkey);
            let close_tp = (uts_sec / self.period_s) * self.period_s;
            if let Some(rec) = self.close_by_fkey.get_mut(&close_fkey) {
                rec.fts_sec = uts_sec;
                rec.close_count += 1;
                rec.camount_sum += famount;
                rec.pa_sum += famount * price;
            } else {
                self.close_by_fkey.insert(
                    close_fkey.clone(),
                    CloseAgg {
                        fts_sec: uts_sec,
                        close_count: 1,
                        camount_sum: famount,
                        pa_sum: famount * price,
                    },
                );
                self.add_close_bucket(close_tp, &close_fkey);
            }
        }

        outputs
    }

    fn add_open_bucket(&mut self, bucket_ts: i64, fkey: &str) {
        self.open_buckets
            .entry(bucket_ts)
            .or_insert_with(HashSet::new)
            .insert(fkey.to_string());
        if self.open_bucket_seen.insert(bucket_ts) {
            self.open_bucket_order.push_back(bucket_ts);
        }
    }

    fn add_close_bucket(&mut self, bucket_ts: i64, fkey: &str) {
        self.close_buckets
            .entry(bucket_ts)
            .or_insert_with(HashSet::new)
            .insert(fkey.to_string());
        if self.close_bucket_seen.insert(bucket_ts) {
            self.close_bucket_order.push_back(bucket_ts);
        }
    }

    fn expire_old(&mut self, current_tp: i64) {
        let expire_before = current_tp - self.max_keep_seconds;
        while let Some(bucket_ts) = self.open_bucket_order.front().copied() {
            if bucket_ts >= expire_before {
                break;
            }
            self.open_bucket_order.pop_front();
            self.open_bucket_seen.remove(&bucket_ts);
            if let Some(fkeys) = self.open_buckets.remove(&bucket_ts) {
                for fkey in fkeys {
                    self.open_by_fkey.remove(&fkey);
                }
            }
        }
        while let Some(bucket_ts) = self.close_bucket_order.front().copied() {
            if bucket_ts >= expire_before {
                break;
            }
            self.close_bucket_order.pop_front();
            self.close_bucket_seen.remove(&bucket_ts);
            if let Some(fkeys) = self.close_buckets.remove(&bucket_ts) {
                for fkey in fkeys {
                    self.close_by_fkey.remove(&fkey);
                }
            }
        }
    }

    fn update_rolling(&mut self, value: Option<f64>) -> Option<f64> {
        self.roll_values.push_back(value);
        if let Some(v) = value {
            self.roll_sum += v;
            self.roll_count += 1;
        }
        if self.roll_values.len() > self.rolling_window {
            if let Some(old) = self.roll_values.pop_front().and_then(|v| v) {
                self.roll_sum -= old;
                if self.roll_count > 0 {
                    self.roll_count -= 1;
                }
            }
        }
        if self.roll_count >= self.min_periods && self.roll_count > 0 {
            Some(self.roll_sum / self.roll_count as f64)
        } else {
            None
        }
    }

    fn finalize_bucket(&mut self, target_ts: i64) -> Option<f64> {
        let fkeys = match self.open_buckets.remove(&target_ts) {
            Some(v) => v,
            None => return None,
        };

        let mut pnlu_sum = 0.0;
        let mut pnlu_count = 0usize;
        for fkey in fkeys {
            let open_rec = match self.open_by_fkey.get(&fkey) {
                Some(v) => v.clone(),
                None => continue,
            };
            let close_rec = match self.close_by_fkey.get(&fkey) {
                Some(v) => v.clone(),
                None => {
                    self.open_by_fkey.remove(&fkey);
                    continue;
                }
            };
            if close_rec.camount_sum <= 0.0 || open_rec.price <= 0.0 {
                self.open_by_fkey.remove(&fkey);
                self.close_by_fkey.remove(&fkey);
                continue;
            }
            let cprice = close_rec.pa_sum / close_rec.camount_sum;
            let pnlu = if open_rec.side == "buy" {
                (cprice - open_rec.price) / open_rec.price
            } else {
                (open_rec.price - cprice) / open_rec.price
            };
            pnlu_sum += pnlu;
            pnlu_count += 1;
            self.open_by_fkey.remove(&fkey);
            self.close_by_fkey.remove(&fkey);
        }

        if pnlu_count > 0 {
            Some(pnlu_sum)
        } else {
            None
        }
    }
}

fn parse_close_fkey(raw_fkey: &str) -> String {
    let parts: Vec<&str> = raw_fkey.split('_').collect();
    if parts.len() >= 3 {
        parts[2].to_string()
    } else {
        raw_fkey.to_string()
    }
}
