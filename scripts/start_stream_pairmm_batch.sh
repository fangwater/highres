#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm_batch.sh [--ipc-prefix <path>] [--name <pm2_name>] [--config <path>] [--highres-config <path>] [--bin <path>] [--log-dir <path>] [--max-log-size-mb <n>] [--max-log-files <n>] [--rotate-check-sec <n>]

Defaults:
  --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures
  --name       stream_pairmm_batch
  --config     <repo>/config.toml           (online_symbols 配置)
  --highres-config <repo>/highres.toml      (策略/引擎配置)
  --log-dir    <repo>/logs/<pm2_name>
  --max-log-size-mb 200
  --max-log-files   10
  --rotate-check-sec 30

Examples:
  ./scripts/start_stream_pairmm_batch.sh
  ./scripts/start_stream_pairmm_batch.sh --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures
  ./scripts/start_stream_pairmm_batch.sh --highres-config ./highres_two_exchange.toml
  ./scripts/start_stream_pairmm_batch.sh --log-dir ./logs/stream_pairmm_batch
  ./scripts/start_stream_pairmm_batch.sh --max-log-size-mb 500 --max-log-files 20
  ./scripts/start_stream_pairmm_batch.sh --name stream_pairmm_batch_main
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

IPC_PREFIX="/tmp/mth_pubs/okex-futures-binance-futures"
NAME="stream_pairmm_batch"
CONFIG_PATH="${BASE_DIR}/config.toml"
HIGHRES_CONFIG_PATH="${BASE_DIR}/highres.toml"
BIN_OVERRIDE=""
LOG_DIR=""
LOG_DIR_SET="0"
MAX_LOG_SIZE_MB="200"
MAX_LOG_FILES="10"
ROTATE_CHECK_SEC="30"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ipc-prefix)
      IPC_PREFIX="${2:-}"
      if [[ -z "$IPC_PREFIX" ]]; then
        echo "[ERROR] --ipc-prefix requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --name)
      NAME="${2:-}"
      if [[ -z "$NAME" ]]; then
        echo "[ERROR] --name requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --config)
      CONFIG_PATH="${2:-}"
      if [[ -z "$CONFIG_PATH" ]]; then
        echo "[ERROR] --config requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --highres-config)
      HIGHRES_CONFIG_PATH="${2:-}"
      if [[ -z "$HIGHRES_CONFIG_PATH" ]]; then
        echo "[ERROR] --highres-config requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --bin)
      BIN_OVERRIDE="${2:-}"
      if [[ -z "$BIN_OVERRIDE" ]]; then
        echo "[ERROR] --bin requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --log-dir)
      LOG_DIR="${2:-}"
      if [[ -z "$LOG_DIR" ]]; then
        echo "[ERROR] --log-dir requires a value" >&2
        usage >&2
        exit 1
      fi
      LOG_DIR_SET="1"
      shift 2
      ;;
    --max-log-size-mb)
      MAX_LOG_SIZE_MB="${2:-}"
      if [[ -z "$MAX_LOG_SIZE_MB" ]]; then
        echo "[ERROR] --max-log-size-mb requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --max-log-files)
      MAX_LOG_FILES="${2:-}"
      if [[ -z "$MAX_LOG_FILES" ]]; then
        echo "[ERROR] --max-log-files requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --rotate-check-sec)
      ROTATE_CHECK_SEC="${2:-}"
      if [[ -z "$ROTATE_CHECK_SEC" ]]; then
        echo "[ERROR] --rotate-check-sec requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[ERROR] Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$LOG_DIR_SET" == "0" ]]; then
  LOG_DIR="${BASE_DIR}/logs/${NAME}"
fi

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] config file not found: ${CONFIG_PATH}" >&2
  exit 1
fi

if [[ ! -f "$HIGHRES_CONFIG_PATH" ]]; then
  echo "[ERROR] highres config file not found: ${HIGHRES_CONFIG_PATH}" >&2
  exit 1
fi

RUNNER_PATH="${SCRIPT_DIR}/stream_pairmm_batch_runner.sh"
if [[ ! -f "$RUNNER_PATH" ]]; then
  echo "[ERROR] runner script not found: ${RUNNER_PATH}" >&2
  exit 1
fi

NAMESPACE="$(basename "${BASE_DIR}")"

echo "[INFO] Restarting ${NAME}"
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

PM2_CMD=(
  pm2 start "$RUNNER_PATH"
  --interpreter bash
  --name "$NAME"
  --namespace "$NAMESPACE"
  --cwd "$BASE_DIR"
  --
  --ipc-prefix "$IPC_PREFIX"
  --config "$CONFIG_PATH"
  --highres-config "$HIGHRES_CONFIG_PATH"
  --log-dir "$LOG_DIR"
  --max-log-size-mb "$MAX_LOG_SIZE_MB"
  --max-log-files "$MAX_LOG_FILES"
  --rotate-check-sec "$ROTATE_CHECK_SEC"
)

if [[ -n "$BIN_OVERRIDE" ]]; then
  PM2_CMD+=(--bin "$BIN_OVERRIDE")
fi

RUST_LOG="${RUST_LOG:-info}" "${PM2_CMD[@]}"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
