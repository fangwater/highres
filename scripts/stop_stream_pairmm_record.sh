#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  stop_stream_pairmm_record.sh [--name <pm2_name>]

Examples:
  ./scripts/stop_stream_pairmm_record.sh
  ./scripts/stop_stream_pairmm_record.sh --name stream_pairmm_record
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
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

pm2 delete "$NAME" --namespace "$NAMESPACE" || true
echo "[INFO] Stopped: ${NAME} (namespace: ${NAMESPACE})"
