# Stream PairMM 配置说明

本说明面向 `stream_pairmm`（策略 stg），强调以 `highres.toml` 为主配置。

## 1. 配置文件位置

部署目录下需要包含：
- `highres.toml`

`stream_pairmm` 会在当前工作目录读取 `highres.toml`。

## 2. 关键配置（强调 stg）

`highres.toml` 的 `engin` 段必须设置对应的策略名称：

```toml
[engin]
stg = "pairmm_two_exchange_simple"   # 使用 stream_pairmm 时，必须是 pairmm_two_exchange_simple 或 pairmm
sids = {0 = {exchange = "okx", etype = "swap"}, 1 = {exchange = "binance", etype = "swap"}}
vsids = [0, 1]          # 必须包含 2 个 sid，用于 origin(0/1) 映射
```

说明：
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

## 4. 启动方式（仅 Batch）

`stream_pairmm` 只消费 IPC 二进制流，不需要 JSON。生产运行统一使用 batch 方式，由 PM2 管理一个总进程。

启动（推荐）：
```bash
./scripts/start_stream_pairmm_batch.sh \
  --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures \
  --log-dir ./logs/stream_pairmm_batch \
  --max-log-size-mb 200 \
  --max-log-files 10 \
  --rotate-check-sec 30
```

停止（batch）：
```bash
./scripts/stop_stream_pairmm_batch.sh
```

## 5. 日志

- PM2 只管理 batch 总进程（可用 `pm2 logs` 看调度日志）。
- 子进程日志按 symbol 分文件，默认目录为仓库根目录下：
  - `logs/stream_pairmm_batch/<SYMBOL>.out.log`
  - `logs/stream_pairmm_batch/<SYMBOL>.err.log`
- 日志会按大小自动轮转，参数由 `--max-log-size-mb / --max-log-files / --rotate-check-sec` 控制。
