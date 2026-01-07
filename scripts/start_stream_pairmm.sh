#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm.sh --ipc <path> [--name <pm2_name>]

Examples:
  ./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc
  ./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc --name stream_pairmm_sol
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

IPC_PATH=""
NAME_OVERRIDE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --ipc)
      IPC_PATH="${2:-}"
      if [[ -z "$IPC_PATH" ]]; then
        echo "[ERROR] --ipc requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --name)
      NAME_OVERRIDE="${2:-}"
      if [[ -z "$NAME_OVERRIDE" ]]; then
        echo "[ERROR] --name requires a value" >&2
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

if [[ -z "$IPC_PATH" ]]; then
  echo "[ERROR] --ipc is required" >&2
  usage >&2
  exit 1
fi

NAMESPACE="$(basename "${BASE_DIR}")"

sanitize_name() {
  echo "$1" | sed -E 's/[^A-Za-z0-9]+/-/g' | sed -E 's/^-+|-+$//g'
}

derive_name_from_ipc() {
  local ipc="$1"
  local file
  file="$(basename "$ipc")"
  local symbol="${file%.*}"
  local pair
  pair="$(basename "$(dirname "$ipc")")"
  local sym_s
  sym_s="$(sanitize_name "$symbol")"
  local pair_s
  pair_s="$(sanitize_name "$pair")"
  if [[ -n "$pair_s" ]]; then
    echo "stream_pairmm-${pair_s}-${sym_s}"
  else
    echo "stream_pairmm-${sym_s}"
  fi
}

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
else
  NAME="$(derive_name_from_ipc "$IPC_PATH")"
fi

BIN_CANDIDATES=(
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

echo "[INFO] Restarting ${NAME}"
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

RUST_LOG="${RUST_LOG:-info}" pm2 start "$BIN_PATH" \
  --name "$NAME" \
  --namespace "$NAMESPACE" \
  --cwd "$BASE_DIR" \
  -- \
  --ipc "$IPC_PATH"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
