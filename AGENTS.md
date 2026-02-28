# Repository Guidelines

## 项目结构与模块组织
本仓库是 Rust 交易回放/流式处理项目，包含多个可执行程序和本地依赖 crate。

- `src/`：核心引擎模块（`lprocess`、`tprocess`、`spending`、`record`、`stg` 等）。
- `src/bin/`：可执行入口（如 `stream_pairmm`、`stream_pairmm_record`、`pnlu_factor_stream`、`pnlu_factor_rolling_metrics`）。
- `src/stgs/`：策略实现（`pairmm`、`pairmm_simple`、`sampling_v6` 等）。
- `markets/`、`mm_common/`、`market_type/`：主包使用的本地 path crate。
- `scripts/`：部署、启动、停止、导出脚本（基于 PM2）。
- `docs/`：格式说明与引擎文档。
- 配置文件：`highres.toml`、`config.toml`、`pnlu_factor.toml`、`pnlu_factor_rolling.toml`。

运行产物通常位于 `logs/`、`data/`、`target/`。

## 构建、测试与开发命令
- `cargo check`：快速编译检查，建议作为日常开发第一步。
- `cargo build --release --bin stream_pairmm`：构建生产版二进制。
- `cargo run --bin stream_pairmm -- --ipc /tmp/mth_pubs/okex-futures-binance-futures/ETHUSDT.ipc`：本地单路流运行示例。
- `cargo test`：运行根 crate 测试。
- `cargo test --manifest-path markets/Cargo.toml`：运行 `markets` 子 crate 测试。
- `bash scripts/start_stream_pairmm.sh --ipc <path>` / `bash scripts/stop_stream_pairmm.sh`：启动/停止 PM2 托管进程。

## 代码风格与命名规范
- Rust 使用 2021 edition、4 空格缩进，并保持 `rustfmt` 默认格式。
- 命名约定：函数/模块/文件用 `snake_case`，类型用 `PascalCase`，常量用 `SCREAMING_SNAKE_CASE`。
- 新增可执行行为优先放在 `src/bin/`，避免让 `src/main.rs` 继续膨胀。
- Shell 脚本保持 `set -euo pipefail`，参数名优先使用长选项。

## 测试指南
- 单元测试尽量就近放置在改动模块内（`#[cfg(test)] mod tests`），命名建议 `test_*`。
- 交易所解析与精度相关改动，优先补充 `markets/src/exchanges/*` 下现有测试。
- 二进制行为改动需补充最小冒烟验证（如 `--help` 或最小配置运行）并在 PR 中写明命令。

## 提交与 Pull Request 规范
- 历史提交以简短祈使句为主（如 `fix ...`、`add ...`），建议保留该风格并加上作用域。
- 推荐格式：`stream_pairmm: handle empty IPC payload`。
- 避免使用 `xx`、`...` 这类无语义信息的提交信息。
- PR 至少包含：
  - 变更内容与动机；
  - 受影响配置/脚本（如 `highres.toml`、PM2 脚本、部署脚本）；
  - 已执行命令（如 `cargo check`、`cargo test`、冒烟命令）；
  - 行为变更对应的日志或输出路径示例。

## 安全与配置提示
- 禁止提交密钥、口令、内网凭据等敏感信息。
- `*_cache.json` 视为可再生成缓存，可用 `python3 scripts/refresh_markets_cache.py` 刷新。
- 在新环境运行前，先检查 TOML 中的绝对路径（大量默认值使用 `/mnt/data` 风格路径）。
