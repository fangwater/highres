# Stream pairmm JSONL Format

This document describes the standard line-oriented JSON input format consumed by `stream_pairmm`.

## Transport

- Input is JSON Lines (one JSON object per line).
- Empty lines or lines starting with `#` are ignored.
- The process is single-symbol: the first `symbol` seen is used for the lifetime of the process. Any other symbol is ignored.
- Time ordering is assumed to be provided by upstream (sorted by time). The engine does not enforce ordering.

## Common Fields

- `symbol` (string, required): e.g. `DOTUSDT`. Used to build `symbol_std` and to tag outputs.
- `sid` (int, required for `inc` and `trade`): exchange/etype id, must be in `vsids` from `highres.toml`.

## Event Types

All events carry `event` as a string discriminator.

### 1) Orderbook Update (`inc`)

```json
{"event":"inc","symbol":"DOTUSDT","ts_us":1000000,"sid":0,"is_snapshot":1,"side_id":0,"price":100.0,"amount":5.0}
```

Fields
- `ts_us` (int): timestamp in microseconds.
- `is_snapshot` (int): 1 for snapshot building rows, 0 for incremental updates.
- `side_id` (int): 0 = bid, 1 = ask.
- `price` (float): price level.
- `amount` (float): size at that price level; `0.0` means remove the level.

Snapshot rule
- Send one or more rows with `is_snapshot=1` to build the snapshot.
- The first row with `is_snapshot=0` after snapshot finalizes the book and enables trading decisions.

### 2) Trade Tick (`trade`)

```json
{"event":"trade","symbol":"DOTUSDT","ts_us":1000500,"sid":0,"side_id":1,"price":99.9,"amount":2.0}
```

Fields
- `ts_us` (int): timestamp in microseconds.
- `side_id` (int): 0 = buy, 1 = sell.
- `price` (float): trade price.
- `amount` (float): trade amount.

### 3) Signal Tick (`tick`)

```json
{"event":"tick","symbol":"DOTUSDT","ts_s":10}
```

Fields
- `ts_s` (int): timestamp in seconds.

## Engine Constraints (pairmm_simple)

- `vsids` controls which sids are active. Events with sids not in `vsids` are ignored.
- Single process should handle only one symbol to avoid shared global state collisions.

## Running

```bash
cargo run --bin stream_pairmm < data/stream_pairmm_sample.jsonl
```

Logs are written to `logs/app.log`.
