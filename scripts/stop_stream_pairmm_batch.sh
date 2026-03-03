#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  stop_stream_pairmm_batch.sh [--name <pm2_name>] [--profile <name>]

Examples:
  ./scripts/stop_stream_pairmm_batch.sh
  ./scripts/stop_stream_pairmm_batch.sh --profile okex-futures-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --profile binance-margin-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --profile binance-futures-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --name stream_pairmm_batch
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
PROFILE=""
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
    --profile)
      PROFILE="${2:-}"
      if [[ -z "$PROFILE" ]]; then
        echo "[ERROR] --profile requires a value" >&2
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

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
elif [[ -n "$PROFILE" ]]; then
  NAME="stream_pairmm_batch-${PROFILE}"
else
  NAME="stream_pairmm_batch"
fi
NAMESPACE="$(basename "${BASE_DIR}")"

pm2 delete "$NAME" --namespace "$NAMESPACE" || true
RECORD_NAME="${NAME}-record"
STOP_RECORD_SCRIPT="${SCRIPT_DIR}/stop_stream_pairmm_record.sh"
if [[ -f "$STOP_RECORD_SCRIPT" ]]; then
  "$STOP_RECORD_SCRIPT" --name "$RECORD_NAME"
else
  pm2 delete "$RECORD_NAME" --namespace "$NAMESPACE" || true
fi

echo "[INFO] Stopped: ${NAME} + ${RECORD_NAME} (namespace: ${NAMESPACE})"
