#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STOP_SCRIPT="${SCRIPT_DIR}/stop_stream_pairmm_batch.sh"
NAME="stream_pairmm_batch_one_exchange"

if [[ ! -f "$STOP_SCRIPT" ]]; then
  echo "[ERROR] script not found: ${STOP_SCRIPT}" >&2
  exit 1
fi

exec "$STOP_SCRIPT" --name "$NAME" "$@"
