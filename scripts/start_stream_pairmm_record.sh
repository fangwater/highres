#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm_record.sh [--name <pm2_name>] [--ipc-prefix <ipc>] [--db-root <path>]

Examples:
  ./scripts/start_stream_pairmm_record.sh
  ./scripts/start_stream_pairmm_record.sh --name stream_pairmm_record
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
IPC_PREFIX=""
DB_ROOT=""
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
    --db-root)
      DB_ROOT="${2:-}"
      if [[ -z "$DB_ROOT" ]]; then
        echo "[ERROR] --db-root requires a value" >&2
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

NAME="${NAME_OVERRIDE:-stream_pairmm_record}"
NAMESPACE="$(basename "${BASE_DIR}")"

BIN_CANDIDATES=(
  "${BASE_DIR}/stream_pairmm_record"
  "${BASE_DIR}/target/release/stream_pairmm_record"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -f "$cand" && -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] stream_pairmm_record binary not found. Build first with: cargo build --release --bin stream_pairmm_record" >&2
  exit 1
fi

ARGS=()
if [[ -n "$IPC_PREFIX" ]]; then
  ARGS+=(--ipc-prefix "$IPC_PREFIX")
fi
if [[ -n "$DB_ROOT" ]]; then
  ARGS+=(--db-root "$DB_ROOT")
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
