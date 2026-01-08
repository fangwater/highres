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
stg = "pairmm_simple"   # 使用 stream_pairmm 时，必须是 pairmm_simple 或 pairmm
sids = {0 = {exchange = "okx", etype = "swap"}, 1 = {exchange = "binance", etype = "swap"}}
vsids = [0, 1]          # 必须包含 2 个 sid，用于 origin(0/1) 映射
```

说明：
- `stg = "pairmm_simple"`：推荐用于实时流（简化版，不依赖因子）。
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

## 4. 启动方式（IPC）

`stream_pairmm` 只消费 IPC 二进制流，不需要 JSON。

示例：
```bash
./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc
```

多进程时可用 `--name` 指定不同的 pm2 名称：
```bash
./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/ETHUSDT.ipc --name stream_pairmm_eth
```

## 5. 日志

`stream_pairmm` 输出到 stdout/stderr，建议通过 PM2 收集日志（`pm2 logs`）。
