# Highres Sampling Two Exchange v6 - Code Walkthrough

This document explains the repository layout, runtime flow, input and output
formats, and the strategy logic implemented in the Rust crate `sampling_v6`.
It is based on a code reading of the repository at
`/home/ubuntu/highres.sampling_two_exchange_v6`.

## 1) Repository layout (high level)

- `src/`: main backtest engine and strategy implementations.
- `highres.toml`: runtime configuration for engine and strategies.
- `log4rs.yaml`: logging configuration for `log4rs`.
- `run.sh`: helper script to clear logs and run `cargo run`.
- `data/`: output folder for CSV dumps.
- `logs/`: log files written by log4rs.
- `markets/` and `market_type/`: local crates for exchange market metadata.
- `standalone_engine/`: a separate JSONL-driven replay/matching engine.
- `*_cache.json`: cached market metadata used by `markets` crate.
- `tools/`: notebooks (not used by the Rust code).

## 2) Build and run

This crate depends on a local path crate `../mm_common`. Make sure it exists.

- Build and run (from repo root):
  - `cargo run`
- Helper script:
  - `./run.sh`

`run.sh` clears `logs/`, kills a named process, and runs `cargo run` under
`nohup`.

### Production deployment (unified entry)

Use unified deployment scripts only:

- Single target:
  - `bash scripts/deploy_stream_pairmm.sh --profile okex-futures-binance-futures`
  - `bash scripts/deploy_stream_pairmm.sh --profile binance-margin-binance-futures`
  - `bash scripts/deploy_stream_pairmm.sh --profile binance-futures-binance-futures`
- All targets:
  - `bash scripts/deploy_all_target.sh`

Split deployment entrypoints were removed; use the unified deploy scripts above.

## 3) Configuration file: `highres.toml`

The `src/gconf.rs` module loads multiple config sections into strongly typed
structs.

### `engin` section (core engine)

- `merged_market_path`: directory for per-hour merged market npy files.
- `tick_path`: directory for per-symbol tick npy files.
- `start_date`, `end_date`: inclusive date range (YYYYMMDD).
- `symbols`: list of symbols (ex: `DOTUSDT`).
- `sids`: map from sid -> {exchange, etype}.
- `vsids`: list of active sids (subset of `sids`).
- `stg`: strategy selector: `pairmm`, `pairmm_one_exchange_simple`, `pairmm_two_exchange_simple`, `sampling`, `sampling_v5`, `sampling_v6`.
- `dump_path`: output folder for CSVs (default `data`).
- `start_ts`, `end_ts`: time filter for ticks (seconds).
- `cancel_delay_ms`: cancel delay used in decisions.
- `is_spending_open_dump`: write open-order CSV rows when true.
- `is_spending_tick_dump`: write periodic tick snapshots to CSV when true.
- `tick_dump_modts`: mod interval for tick snapshot writes.
- `is_target_sid_open_keep`: enable target sid open tracking.

### `pairmm` section (used by pairmm/sampling variants)

- `amountu`: unit size (notional) for order sizing.
- `max_open_order_keep_s`, `max_close_order_keep_s`: max open time for orders.
- `open_ranges`: price offsets from bid1/ask1 for opening orders.
- `close_rb`: close price offset.
- `close_ts`: wait time before closing.
- `sample_step`: tick sampling interval (seconds).
- `is_start_close_open`: choose how close timing is calculated.
- `tlenu`: min depth value to allow open.
- `fdf`: threshold for factor feature gating.
- `is_opne_price_gap_ratio`, `opne_price_gap_ratio`: optional cancel rule.
- `opne_sid`: open only on this sid (used in sampling_v5/v6).
- `max_pos_u`: max notional position limit.

Other sections (`arbmm`, `arbmm2`, `arbmt_sampling`, `nmt`) are loaded but the
code paths are not wired into `src/stg.rs` in this repository.

## 4) Input data formats

### 4.1 Tick data (`tick_path/{symbol}.npy`)

Loaded in `src/main.rs` and passed to `stg::tick()` as a `Vec<f64>`.

Common assumptions from code:

- Column 0 is the tick timestamp in seconds (`t[0]`).
- Additional columns are strategy specific:
  - `sampling`: only uses `ts`.
  - `sampling_v5`: `ts`, `signal1`, `signal2`, `signal3`.
  - `sampling_v6`: `ts`, `signal1`, `signal2`, `signal3`, `buy_cancel`,
    `sell_cancel`.
  - `pairmm`: expects factor columns like `f_0_1`, `f_1_0` (see `COLS` map).
  - `pairmm_one_exchange_simple`: only uses `ts` (no factor columns).
  - `pairmm_two_exchange_simple`: only uses `ts` (no factor columns).

### 4.2 Merged market data (`merged_market_path/{symbol}_{YYYYMMDD}_{HH}.npy`)

Loaded in `src/main.rs` and processed by:

- `lprocess::process()` when `row[5] == 1.0` (orderbook row).
- `tprocess::process()` when `row[5] == 0.0` (trade row).

Column usage (from `lprocess.rs` and `tprocess.rs`):

- `0`: timestamp in microseconds (compared against `tick_ts * 1_000_000`).
- `1`: `is_snapshot` flag for orderbook rows (`1.0` = snapshot).
- `2`: side id (`0` = buy/bid, `1` = sell/ask).
- `3`: price.
- `4`: amount.
- `5`: event type flag (`1.0` = orderbook, `0.0` = trade).
- `6`: `sid` (exchange id).

## 5) Output data formats

### 5.1 Order records: `data/{symbol}_orders.csv`

Written by `src/record.rs::write_to_csv`. Each row is a `RecordDumpItem`.
Column order is:

1. `client_order_id`
2. `create_ts`
3. `update_ts`
4. `client_order_id` (duplicated in code)
5. `symbol`
6. `ttype` (maker or taker)
7. `sid`
8. `side` (buy or sell)
9. `price`
10. `amount_init`
11. `amount_update`
12. `status` (open, filled, partial_filled, canceled, tickupdate, etc)
13. `inpos`
14. `tlen`
15. `from_key`
16. `bid1`
17. `ask1`

### 5.2 Position snapshots: `data/{symbol}_nps.csv`

Written by `write_to_csv_ts`. Column order:

1. `create_ts`
2. `symbol`
3. `sid`
4. `pos`
5. `open`

### 5.3 CSV splitting

After each symbol finishes, `main` calls `outsplit::split_csv_file` to split
`{symbol}_orders.csv` into 10 smaller files with prefix `{symbol}_orders_`.

## 6) Runtime flow (main engine)

The backtest loop is in `src/main.rs`.

1. Initialize logging (`log4rs.yaml`).
2. For each `symbol`:
   - Build a `TradeInfo` object (depths and positions per sid).
   - Load tick npy for the symbol and iterate rows.
   - For each date in `[start_date, end_date]` and each hour (00-23):
     - Load the corresponding merged market npy (if exists).
     - Interleave:
       - Orderbook updates (`lprocess`) to keep book state.
       - Trade ticks (`tprocess`) to match pending maker orders.
     - On each tick timestamp:
       - Call `stg::tick()` to generate `MakeDecision` and `CancelDecision`.
       - Convert decisions into pending orders (`spending::add_pending`) or
         taker fills (`ordertake::add_taking`).
       - Apply cancels (`spending::drop_pending`).
       - Optionally dump tick snapshots.
3. After each symbol:
   - Clear pending orders and split CSV output.

## 7) Core modules and responsibilities

### `src/trade.rs`

Defines core data structures:

- `TradeInfo`: per-symbol state, including depth and positions.
- `DepthInfo`: per-sid orderbook data.
- `MakeDecision` and `CancelDecision`: outputs from strategies.

### `src/symbolinfo.rs`

Loads exchange market metadata via `markets::fetch_markets`.
Applies exchange-specific corrections:

- `gate` linear swap: derive lot size from quantity limits.
- `binance` swap: override tick size from `PRICE_FILTER`.

### `src/lprocess.rs` (orderbook updates)

Updates per-sid orderbook and uses `pending_adjust` to update each pending
maker order's `inpos`, `backlen`, and `tlen` based on depth changes. It also
maintains `bid1` and `ask1` once a snapshot is complete.

### `src/tprocess.rs` (trade matching)

Simulates trade ticks hitting maker orders:

- Buy trades consume ask-side pending orders.
- Sell trades consume bid-side pending orders.

On fills it updates positions, writes CSV rows, and calls strategy callbacks
`cb_filled` and `cb_finished`.

### `src/spending.rs` (pending orders)

Creates and manages pending maker orders:

- Price rounding to tick size.
- Avoid crossing the spread (skip if price >= ask1 or price <= bid1).
- Tracks queue position (`inpos`, `backlen`, `tlen`) and consumed amount.

### `src/ordertake.rs` (taker fills)

Implements immediate fills against the current orderbook to simulate taker
orders. It computes an average fill price and updates position/open.

### `src/record.rs`

Writes CSV outputs for orders and position snapshots.

### `src/outsplit.rs`

Splits large CSVs into smaller files using `wc`, `split`, and `sed`.

### `src/target_sid_open.rs`

Optional tracking of "open notional" per sid when
`is_target_sid_open_keep = true`.

### `src/stg.rs`

Strategy dispatcher:

- `cb_init`, `cb_filled`, `cb_finished`
- `make` and `cancel` decisions

The actual strategy is selected by `engin.stg` in `highres.toml`.

## 8) Strategies

All strategies implement:

- `cb_init(tinfo)`
- `cb_filled(tinfo, pending_item, filled_amount)`
- `cb_finished(ts, tinfo, pending_item, is_cancel)`
- `make(tick, tinfo) -> Vec<MakeDecision>`
- `cancel(tick, tinfo) -> Vec<CancelDecision>`

### 8.1 `pairmm` (`src/stgs/pairmm.rs`)

Pair market-making with open and close legs:

- Uses `open_ranges` to place maker orders around bid1/ask1.
- Uses factor columns (example `f_0_1`, `f_1_0`) from tick data.
- Requires sufficient depth at target prices (`tlenu` threshold).
- Tracks open positions and generates close orders via `ONGOING_BID` and
  `ONGOING_ASK`.
- Cancels open orders after `max_open_order_keep_s`.
- Cancels close orders after `max_close_order_keep_s`.

Close pricing uses `close_rb` and `close_ts`.

### 8.2 `pairmm_one_exchange_simple` (`src/stgs/pairmm_one_exchange_simple.rs`)

Single-exchange simplified pair market-making (migrated from `pairmm.rs.x`):

- Same open/close flow as `pairmm`.
- No factor-gating (`fdf`) and no depth thresholding (`tlenu`).
- Tick input only needs `ts`.

### 8.3 `pairmm_two_exchange_simple` (`src/stgs/pairmm_two_exchange_simple.rs`)

Simplified pair market-making:

- Same open/close flow as `pairmm`.
- Removes factor-gating (`fdf`) and depth thresholding (`tlenu`).
- Tick input only needs `ts`.

### 8.4 `sampling` (`src/stgs/sampling.rs`)

Time-based sampling strategy:

- Only uses tick `ts`.
- Every `sample_step` seconds it emits maker orders with `open_ranges`.
- Close logic and order tracking mirrors `pairmm`.

### 8.5 `sampling_v5` (`src/stgs/sampling_v5.rs`)

Signal-driven sampling:

- Tick columns: `ts`, `signal1`, `signal2`, `signal3`.
- Only opens on the configured `opne_sid`.
- Open conditions:
  - Buy: `signal1 == 1` and `signal2 == 1` and `signal3 == 1`.
  - Sell: `signal1 == -1` and `signal2 == 1` and `signal3 == 1`.
- Price offsets use `open_ranges` and current bid1/ask1.

Close logic mirrors `pairmm`.

### 8.6 `sampling_v6` (`src/stgs/sampling_v6.rs`)

Adds cancel flags to `sampling_v5`:

- Tick columns: `ts`, `signal1`, `signal2`, `signal3`, `buy_cancel`, `sell_cancel`.
- Open conditions are the same as `sampling_v5`.
- Cancel behavior:
  - Open bid orders can be canceled when `buy_cancel == 1`.
  - Open ask orders can be canceled when `sell_cancel == 1`.

Close logic mirrors `pairmm`.

## 9) Standalone engine (`standalone_engine/`)

This is a separate crate that provides a single-threaded JSONL replay and
matching engine. It re-implements the key `lprocess`/`tprocess` logic with a
clean event API.

- Input events: `init`, `ob`, `trade`, `own_new`, `own_cancel`, `tick`,
  `sample`, `clear`.
- Output events: `fill`, `summary`, `ack`.
- Read `standalone_engine/README.md` for details.

Use it for unit testing or small-scale simulation without the full npy
pipeline.

## 10) Notes and assumptions

- Tick timestamps are treated as seconds; merged-market timestamps are treated
  as microseconds.
- The strategy code is tightly coupled to the input column order. If your
  npy files differ, adjust the `COLS` maps.
- The repository contains additional strategy files (`arbmm`, `arbmm2`, etc)
  that are not wired into `src/stg.rs` in this version.
