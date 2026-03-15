# Stream PairMM 配置说明

本说明面向 `stream_pairmm`（策略 stg），强调以 `highres.toml` 为主配置。

## 1. 配置文件位置

部署目录下需要包含：
- `highres.toml`
- `config.toml`
- `pnlu_factor.toml`
- `pnlu_factor_rolling_symbols.json`

`stream_pairmm` 会在当前工作目录读取 `highres.toml`。
`pnlu_factor_stream` 会在同一进程内完成因子计算和 rolling/threshold 写 Redis。
`pnlu_factor.toml` 同时承载 factor 参数和 rolling/Redis 参数，不再使用独立的 `pnlu_factor_rolling.toml`。

## 1.1 Pnlu Profile 配置

三套 profile 可以各自维护独立的 pnlu 配置文件：

- `pnlu_factor.okex-futures-binance-futures.toml`
- `pnlu_factor.binance-margin-binance-futures.toml`
- `pnlu_factor.binance-futures-binance-futures.toml`

对应的 rolling symbol 配置也可以独立：

- `pnlu_factor_rolling_symbols.okex-futures-binance-futures.json`
- `pnlu_factor_rolling_symbols.binance-margin-binance-futures.json`
- `pnlu_factor_rolling_symbols.binance-futures-binance-futures.json`

部署时固定同步到对应的 `stream_pairmm` 目录，并覆盖该目录内的：

- `pnlu_factor.toml`
- `pnlu_factor_rolling_symbols.json`

`pnlu_factor_stream` 启动后直接读取当前目录的：

- `config.toml`
- `pnlu_factor.toml`
- `pnlu_factor_rolling_symbols.json`

因此 symbol 集合与 `stream_pairmm` 使用的 `config.toml` 严格一一对应。
启动和停止也固定按 `--profile` 执行，不再支持自定义 pnlu 配置路径或自定义目标目录。

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

Pnlu 独立部署：

```bash
# 固定部署到对应的 stream_pairmm profile 目录
bash scripts/deploy_pnlu_factor_stream.sh --profile okex-futures-binance-futures
bash scripts/deploy_pnlu_factor_stream.sh --profile binance-margin-binance-futures
bash scripts/deploy_pnlu_factor_stream.sh --profile binance-futures-binance-futures

# 一次部署全部三套
bash scripts/deploy_pnlu_factor_stream.sh --all
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
- 子进程日志由 `stream_pairmm` 自己写到 `/mnt/data/stream_pairmm/<profile>/<SYMBOL>.log`。
- `/mnt` 保证存在；其余目录不存在时会自动递归创建。
- 单个日志文件超过 20MB 后会直接删除旧文件并从空文件继续写，只保留最新日志内容。
