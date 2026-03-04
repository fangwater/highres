#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  stream_pairmm_batch_runner.sh [--ipc-prefix <path>] [--config <path>] [--highres-config <path>] [--bin <path>] [--log-dir <path>] [--max-log-size-mb <n>] [--max-log-files <n>] [--rotate-check-sec <n>]

Defaults:
  --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures
  --config     <repo>/config.toml       (online_symbols 配置)
  --highres-config <repo>/highres.toml  (策略/引擎配置)
  --bin        auto detect (<repo>/stream_pairmm or <repo>/target/release/stream_pairmm)
  --log-dir    <repo>/logs/stream_pairmm_batch
  --max-log-size-mb 200
  --max-log-files   10
  --rotate-check-sec 30
EOF
}

IPC_PREFIX="/tmp/mth_pubs/okex-futures-binance-futures"
CONFIG_PATH="${BASE_DIR}/config.toml"
HIGHRES_CONFIG_PATH="${BASE_DIR}/highres.toml"
BIN_OVERRIDE=""
LOG_DIR="${BASE_DIR}/logs/stream_pairmm_batch"
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

if ! [[ "$MAX_LOG_SIZE_MB" =~ ^[0-9]+$ ]] || [[ "$MAX_LOG_SIZE_MB" -le 0 ]]; then
  echo "[ERROR] --max-log-size-mb must be a positive integer" >&2
  exit 1
fi
if ! [[ "$MAX_LOG_FILES" =~ ^[0-9]+$ ]] || [[ "$MAX_LOG_FILES" -le 0 ]]; then
  echo "[ERROR] --max-log-files must be a positive integer" >&2
  exit 1
fi
if ! [[ "$ROTATE_CHECK_SEC" =~ ^[0-9]+$ ]] || [[ "$ROTATE_CHECK_SEC" -le 0 ]]; then
  echo "[ERROR] --rotate-check-sec must be a positive integer" >&2
  exit 1
fi

MAX_LOG_SIZE_BYTES=$((MAX_LOG_SIZE_MB * 1024 * 1024))

mkdir -p "$LOG_DIR"

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
declare -a LOG_FILES=()
ROTATOR_PID=""

rotate_file_copytruncate() {
  local file="$1"
  local i=0
  local prev=0

  if [[ ! -f "$file" ]]; then
    return 0
  fi

  i="$MAX_LOG_FILES"
  while [[ "$i" -ge 2 ]]; do
    prev=$((i - 1))
    if [[ -f "${file}.${prev}" ]]; then
      mv -f "${file}.${prev}" "${file}.${i}" || true
    fi
    i=$((i - 1))
  done

  cp -f "$file" "${file}.1" || true
  : > "$file"
}

rotate_logs_loop() {
  local file=""
  local size=0
  while true; do
    sleep "$ROTATE_CHECK_SEC"
    for file in "${LOG_FILES[@]}"; do
      if [[ ! -f "$file" ]]; then
        continue
      fi
      size="$(stat -c%s "$file" 2>/dev/null || echo 0)"
      if [[ "$size" -ge "$MAX_LOG_SIZE_BYTES" ]]; then
        echo "[INFO] rotate log file=${file} size=${size} threshold=${MAX_LOG_SIZE_BYTES}"
        rotate_file_copytruncate "$file"
      fi
    done
  done
}

stop_rotator() {
  if [[ -n "$ROTATOR_PID" ]] && kill -0 "$ROTATOR_PID" >/dev/null 2>&1; then
    kill "$ROTATOR_PID" >/dev/null 2>&1 || true
    wait "$ROTATOR_PID" >/dev/null 2>&1 || true
  fi
}

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
  stop_rotator
  stop_children
  exit 0
}

trap on_signal INT TERM HUP

for symbol in $symbols_raw; do
  ipc_path="${IPC_ROOT}/${symbol}.ipc"
  out_log="${LOG_DIR}/${symbol}.out.log"
  err_log="${LOG_DIR}/${symbol}.err.log"
  echo "[INFO] start child symbol=${symbol} ipc=${ipc_path} out=${out_log} err=${err_log}"

  {
    echo ""
    echo "===== $(date '+%F %T') start symbol=${symbol} ipc=${ipc_path} highres=${HIGHRES_CONFIG_PATH} ====="
  } >>"$out_log"
  {
    echo ""
    echo "===== $(date '+%F %T') start symbol=${symbol} ipc=${ipc_path} highres=${HIGHRES_CONFIG_PATH} ====="
  } >>"$err_log"

  HIGHRES_CONFIG_PATH="$HIGHRES_CONFIG_PATH" RUST_LOG="${RUST_LOG:-info}" "$BIN_PATH" --ipc "$ipc_path" >>"$out_log" 2>>"$err_log" &
  child_pid="$!"
  CHILD_PIDS+=("$child_pid")
  CHILD_SYMBOLS["$child_pid"]="$symbol"
  LOG_FILES+=("$out_log" "$err_log")
done

echo "[INFO] stream_pairmm batch runner started (${#CHILD_PIDS[@]} children), log_dir=${LOG_DIR}"
echo "[INFO] highres config: ${HIGHRES_CONFIG_PATH}"
echo "[INFO] log rotate enabled: max_size_mb=${MAX_LOG_SIZE_MB} keep_files=${MAX_LOG_FILES} check_sec=${ROTATE_CHECK_SEC}"

rotate_logs_loop &
ROTATOR_PID="$!"

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
stop_rotator
stop_children
exit 1
