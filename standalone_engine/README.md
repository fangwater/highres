# standalone_engine

单线程（`tokio` current-thread）行情回放/撮合小进程：从 stdin 读取 JSONL 事件，内部用 `lprocess` 更新盘口、用 `tprocess` 用成交去撮合你的挂单队列，并输出 fill/summary JSONL。

## Run

在 `standalone_engine/` 目录：

- 运行样例：`cargo run --offline -- < examples/sample.jsonl`
- 每 N 条输入输出一次 summary：`cargo run --offline -- --sample-every 100 --summary-sid 1 < your.jsonl`

首次编译需要下载依赖（需要网络）：`cargo test` / `cargo build`。

## Input events (JSON lines)

- `init`
  - `{"event":"init","markets":{"1":{"tick_size":0.1,"contract_value":1.0}}}`
- `ob`（orderbook 行：快照/增量）
  - `{"event":"ob","ts_us":123,"sid":1,"is_snapshot":true,"side":"bid","price":100.0,"amount":10.0}`
  - `{"event":"ob","ts_us":124,"sid":1,"is_snapshot":false,"side":"ask","price":101.0,"amount":12.0}`
- `trade`（成交 tick）
  - `{"event":"trade","ts_us":200,"sid":1,"side":"buy","price":101.0,"amount":5.0}`
- `own_new`（喂你的挂单）
  - `{"event":"own_new","ts_us":300,"sid":1,"client_order_id":"o1","side":"sell","price":101.0,"amount":1.0}`
- `own_cancel`
  - `{"event":"own_cancel","client_order_id":"o1"}`
- `sample` / `clear`
  - `{"event":"sample"}` / `{"event":"clear"}`

