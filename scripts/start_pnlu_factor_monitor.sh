#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SUPPORTED_PROFILES=(
  "okex-futures-binance-futures"
  "okex-futures-okex-futures"
  "binance-margin-binance-futures"
  "binance-futures-binance-futures"
)

is_supported_profile() {
  local profile="$1"
  local item
  for item in "${SUPPORTED_PROFILES[@]}"; do
    if [[ "$item" == "$profile" ]]; then
      return 0
    fi
  done
  return 1
}

usage() {
  cat <<'EOF'
Usage:
  start_pnlu_factor_monitor.sh --profile <name> [--host <host>] [--port <port>] [--offline-after <seconds>]

Examples:
  ./scripts/start_pnlu_factor_monitor.sh --profile okex-futures-binance-futures
  ./scripts/start_pnlu_factor_monitor.sh --profile okex-futures-okex-futures
  ./scripts/start_pnlu_factor_monitor.sh --profile okex-futures-binance-futures --host 0.0.0.0 --port 8765
EOF
}

PROFILE=""
HOST="0.0.0.0"
PORT="8765"
OFFLINE_AFTER="10"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --host)
      HOST="${2:-}"
      shift 2
      ;;
    --port)
      PORT="${2:-}"
      shift 2
      ;;
    --offline-after)
      OFFLINE_AFTER="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[ERROR] unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required" >&2
  usage >&2
  exit 1
fi

if ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

cd "$BASE_DIR"
PYTHON_BIN="${MONITOR_PYTHON:-}"
if [[ -z "$PYTHON_BIN" ]]; then
  if [[ -x "${BASE_DIR}/.venv-monitor/bin/python3" ]]; then
    PYTHON_BIN="${BASE_DIR}/.venv-monitor/bin/python3"
  else
    PYTHON_BIN="python3"
  fi
fi

exec env PYTHONUNBUFFERED=1 "$PYTHON_BIN" scripts/pnlu_factor_monitor.py \
  --profile "$PROFILE" \
  --host "$HOST" \
  --port "$PORT" \
  --offline-after "$OFFLINE_AFTER"
