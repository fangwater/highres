# Stream PairMM 配置说明

本说明面向 `stream_pairmm`（策略 stg），强调以 `highres.toml` 为主配置。

## 1. 配置文件位置

部署目录下需要包含：
- `highres.toml`
- `config.toml`
- `pnlu_factor.toml`
- `pnlu_factor_rolling.toml`
- `pnlu_factor_rolling_symbols.json`

`stream_pairmm` 会在当前工作目录读取 `highres.toml`。

## 2. 关键配置（强调 stg）

`highres.toml` 的 `engin` 段必须设置对应的策略名称：

```toml
[engin]
stg = "pairmm_two_exchange_simple"   # 使用 stream_pairmm 时，建议是 pairmm_two_exchange_simple 或 pairmm_one_exchange_simple（也可 pairmm）
sids = {0 = {exchange = "okx", etype = "swap"}, 1 = {exchange = "binance", etype = "swap"}}
vsids = [0, 1]          # 必须包含 2 个 sid，用于 origin(0/1) 映射
```

说明：
- `stg = "pairmm_one_exchange_simple"`：单交易所简化版（由 `pairmm.rs.x` 迁移）。
- `stg = "pairmm_two_exchange_simple"`：推荐用于实时流（简化版，不依赖因子）。
- 如需完整 `pairmm`，可改为 `stg = "pairmm"`，并在 `[pairmm]` 中补齐参数。

## 3. PairMM 参数

`[pairmm]` 区域用于开平仓参数与阈值，例如：

```toml
[pairmm]
open_ranges = [0.0002, 0.0004, 0.0006]
close_rb = 0.0003
amountu = 50
max_open_order_keep_s = 120
max_close_order_keep_s = 30
```

## 4. 统一部署入口

生产环境统一使用以下入口脚本：

```bash
# 部署单套（双所）
bash scripts/deploy_stream_pairmm.sh --profile okex-futures-binance-futures
bash scripts/deploy_stream_pairmm.sh --profile binance-margin-binance-futures

# 部署单套（单所）
bash scripts/deploy_stream_pairmm.sh --profile binance-futures-binance-futures

# 一次部署全部三套
bash scripts/deploy_stream_pairmm.sh --all
```

说明：单所 profile 采用 repeat 形式（同一上游重复两次），例如 `binance-futures-binance-futures`。

## 5. 启动方式（仅 Batch）

`stream_pairmm` 只消费 IPC 二进制流，不需要 JSON。运行统一使用 batch，由 PM2 管理一个总进程。

启动：
```bash
# 一次启动全部三套（推荐）
bash scripts/start_stream_pairmm_batch.sh --all

# 按 profile 启动
bash scripts/start_stream_pairmm_batch.sh --profile okex-futures-binance-futures
bash scripts/start_stream_pairmm_batch.sh --profile binance-margin-binance-futures
bash scripts/start_stream_pairmm_batch.sh --profile binance-futures-binance-futures
```

停止：
```bash
# 一次停止全部三套（推荐）
bash scripts/stop_stream_pairmm_batch.sh --all

# 按 profile 停止
bash scripts/stop_stream_pairmm_batch.sh --profile okex-futures-binance-futures
bash scripts/stop_stream_pairmm_batch.sh --profile binance-margin-binance-futures
bash scripts/stop_stream_pairmm_batch.sh --profile binance-futures-binance-futures
```

## 6. 日志

- PM2 只管理 batch 总进程（可用 `pm2 logs` 看调度日志）。
- 子进程日志按 symbol 分文件，默认目录按进程名隔离：
  - `logs/stream_pairmm_batch-okex-futures-binance-futures/<SYMBOL>.out.log`
  - `logs/stream_pairmm_batch-okex-futures-binance-futures/<SYMBOL>.err.log`
  - `logs/stream_pairmm_batch-binance-margin-binance-futures/<SYMBOL>.out.log`
  - `logs/stream_pairmm_batch-binance-margin-binance-futures/<SYMBOL>.err.log`
  - `logs/stream_pairmm_batch-binance-futures-binance-futures/<SYMBOL>.out.log`
  - `logs/stream_pairmm_batch-binance-futures-binance-futures/<SYMBOL>.err.log`
- 日志会按大小自动轮转，参数由 `--max-log-size-mb / --max-log-files / --rotate-check-sec` 控制。
