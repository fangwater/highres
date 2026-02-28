#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  start_pnlu_factor_stream.sh [--name <pm2_name>] [--ipc-prefix <ipc>] [--config <path>]

Examples:
  ./scripts/start_pnlu_factor_stream.sh
  ./scripts/start_pnlu_factor_stream.sh --ipc-prefix /tmp/mth_pubs/stream_pairmm/okex-futures-binance-futures
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
IPC_PREFIX=""
CONFIG_PATH=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)
      NAME_OVERRIDE="${2:-}"
      if [[ -z "$NAME_OVERRIDE" ]]; then
        echo "[ERROR] --name requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
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

NAME="${NAME_OVERRIDE:-pnlu_factor_stream}"
NAMESPACE="$(basename "${BASE_DIR}")"

BIN_CANDIDATES=(
  "${BASE_DIR}/pnlu_factor_stream"
  "${BASE_DIR}/target/release/pnlu_factor_stream"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -f "$cand" && -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] pnlu_factor_stream binary not found. Build first with: cargo build --release --bin pnlu_factor_stream" >&2
  exit 1
fi

ARGS=()
if [[ -n "$IPC_PREFIX" ]]; then
  ARGS+=(--ipc-prefix "$IPC_PREFIX")
fi
if [[ -n "$CONFIG_PATH" ]]; then
  ARGS+=(--config "$CONFIG_PATH")
fi

echo "[INFO] Restarting ${NAME}"
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

PM2_CMD=(pm2 start "$BIN_PATH" --name "$NAME" --namespace "$NAMESPACE" --cwd "$BASE_DIR")
if [[ ${#ARGS[@]} -gt 0 ]]; then
  PM2_CMD+=(-- "${ARGS[@]}")
fi

RUST_LOG="${RUST_LOG:-info}" "${PM2_CMD[@]}"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
