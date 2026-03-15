# Highres

## Pnlu Redis 写入规则

`pnlu_factor_stream` 按 symbol 将 rolling threshold 结果写入 Redis。

统一规则如下：

- Redis key 的统一后缀为 `_pnlu_factor_thresholds_<profile>`
- 最终 Redis key 规则为 `<SYMBOL>_pnlu_factor_thresholds_<profile>`
- 其中 `<profile>` 与部署 profile 一一对应，当前固定为：
  - `okex-futures-binance-futures`
  - `binance-margin-binance-futures`
  - `binance-futures-binance-futures`

示例：

- `ETHUSDT_pnlu_factor_thresholds_okex-futures-binance-futures`
- `ETHUSDT_pnlu_factor_thresholds_binance-margin-binance-futures`
- `ETHUSDT_pnlu_factor_thresholds_binance-futures-binance-futures`

该规则的目的：

- 同一 symbol 在不同 profile 下写入不同 key，不会互相覆盖
- Redis key 可直接从 `symbol + profile` 推导，无需额外映射表
- 部署目录、进程 profile、Redis key 后缀保持一致，便于排查和交付

Redis value 为 JSON 字符串，字段包括：

- `symbol`
- `ts`
- `target_ts`
- `factor`
- `quantiles`
- `thresholds`
- `ready`

当 `factor` 尚未形成时，不会写 Redis。
