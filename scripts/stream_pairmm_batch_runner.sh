#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  stream_pairmm_batch_runner.sh [--ipc-prefix <path>] [--config <path>] [--highres-config <path>] [--bin <path>]

Defaults:
  --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures
  --config     <repo>/config.toml       (online_symbols 配置)
  --highres-config <repo>/highres.toml  (策略/引擎配置)
  --bin        auto detect (<repo>/stream_pairmm or <repo>/target/release/stream_pairmm)
EOF
}

IPC_PREFIX="/tmp/mth_pubs/okex-futures-binance-futures"
CONFIG_PATH="${BASE_DIR}/config.toml"
HIGHRES_CONFIG_PATH="${BASE_DIR}/highres.toml"
BIN_OVERRIDE=""
PAIRMM_LOG_ROOT="/mnt/data/stream_pairmm"

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

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] config file not found: ${CONFIG_PATH}" >&2
  exit 1
fi

if [[ ! -f "$HIGHRES_CONFIG_PATH" ]]; then
  echo "[ERROR] highres config file not found: ${HIGHRES_CONFIG_PATH}" >&2
  exit 1
fi

IPC_ROOT="${IPC_PREFIX#ipc://}"
IPC_ROOT="${IPC_ROOT%/}"
if [[ -z "$IPC_ROOT" ]]; then
  echo "[ERROR] invalid --ipc-prefix: ${IPC_PREFIX}" >&2
  exit 1
fi

BIN_CANDIDATES=()
if [[ -n "$BIN_OVERRIDE" ]]; then
  BIN_CANDIDATES+=("$BIN_OVERRIDE")
fi
BIN_CANDIDATES+=(
  "${BASE_DIR}/stream_pairmm"
  "${BASE_DIR}/target/release/stream_pairmm"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] stream_pairmm binary not found. Build first with: cargo build --release --bin stream_pairmm" >&2
  exit 1
fi

symbols_raw="$(python3 - "$CONFIG_PATH" <<'PY'
import re
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text(encoding="utf-8", errors="ignore")
symbols = []

def find_key(obj, key):
    if isinstance(obj, dict):
        if key in obj:
            return obj[key]
        for v in obj.values():
            res = find_key(v, key)
            if res is not None:
                return res
    if isinstance(obj, list):
        for v in obj:
            res = find_key(v, key)
            if res is not None:
                return res
    return None

toml = None
try:
    import tomllib as toml
except Exception:
    try:
        import tomli as toml  # type: ignore
    except Exception:
        toml = None

if toml is not None:
    try:
        data = toml.loads(text)
        value = find_key(data, "online_symbols")
        if isinstance(value, list):
            symbols = [str(x) for x in value if str(x).strip()]
    except Exception:
        symbols = []

if not symbols:
    text = re.sub(r"#.*", "", text)
    m = re.search(r"online_symbols\\s*=\\s*\\[(.*?)\\]", text, re.S)
    if m:
        items = [s.strip() for s in m.group(1).split(",") if s.strip()]
        symbols = [s.strip('"').strip("'") for s in items if s.strip()]

print(" ".join(symbols))
PY
)"

if [[ -z "$symbols_raw" ]]; then
  echo "[ERROR] online_symbols is empty in ${CONFIG_PATH}" >&2
  exit 1
fi

declare -a CHILD_PIDS=()
declare -A CHILD_SYMBOLS=()

stop_children() {
  local pid
  for pid in "${CHILD_PIDS[@]}"; do
    if kill -0 "$pid" >/dev/null 2>&1; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done

  for pid in "${CHILD_PIDS[@]}"; do
    wait "$pid" >/dev/null 2>&1 || true
  done
}

on_signal() {
  echo "[INFO] signal received, stopping all stream_pairmm children"
  stop_children
  exit 0
}

trap on_signal INT TERM HUP

PROFILE_NAME="$(basename "$IPC_ROOT")"

for symbol in $symbols_raw; do
  ipc_path="${IPC_ROOT}/${symbol}.ipc"
  echo "[INFO] start child symbol=${symbol} ipc=${ipc_path} log_root=${PAIRMM_LOG_ROOT} profile=${PROFILE_NAME}"

  HIGHRES_CONFIG_PATH="$HIGHRES_CONFIG_PATH" \
  PAIRMM_LOG_ROOT="$PAIRMM_LOG_ROOT" \
  PAIRMM_LOG_PROFILE="$PROFILE_NAME" \
  PAIRMM_LOG_SYMBOL="$symbol" \
  RUST_LOG="${RUST_LOG:-info}" \
  "$BIN_PATH" --ipc "$ipc_path" &
  child_pid="$!"
  CHILD_PIDS+=("$child_pid")
  CHILD_SYMBOLS["$child_pid"]="$symbol"
done

echo "[INFO] stream_pairmm batch runner started (${#CHILD_PIDS[@]} children)"
echo "[INFO] highres config: ${HIGHRES_CONFIG_PATH}"
echo "[INFO] child log root: ${PAIRMM_LOG_ROOT}/${PROFILE_NAME}"

set +e
wait -n
child_code=$?
set -e

exited_pid=""
for pid in "${CHILD_PIDS[@]}"; do
  if ! kill -0 "$pid" >/dev/null 2>&1; then
    exited_pid="$pid"
    break
  fi
done

if [[ -n "$exited_pid" ]]; then
  exited_symbol="${CHILD_SYMBOLS[$exited_pid]:-unknown}"
  echo "[ERROR] child exited symbol=${exited_symbol} pid=${exited_pid} code=${child_code}, stopping remaining children"
else
  echo "[ERROR] child exited (code=${child_code}), stopping remaining children"
fi
stop_children
exit 1
