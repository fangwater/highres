#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

START_SCRIPT="${SCRIPT_DIR}/start_stream_pairmm_batch.sh"
NAME="stream_pairmm_batch_two_exchange"
IPC_PREFIX="/tmp/mth_pubs/okex-futures-binance-futures"
CONFIG_PATH="${BASE_DIR}/config_two_exchange.toml"
HIGHRES_CONFIG_PATH="${BASE_DIR}/highres_two_exchange.toml"

if [[ ! -f "$START_SCRIPT" ]]; then
  echo "[ERROR] script not found: ${START_SCRIPT}" >&2
  exit 1
fi

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] config file not found: ${CONFIG_PATH}" >&2
  echo "[HINT] create it first (example): cp ${BASE_DIR}/config.toml ${CONFIG_PATH}" >&2
  exit 1
fi

if [[ ! -f "$HIGHRES_CONFIG_PATH" ]]; then
  echo "[ERROR] highres config file not found: ${HIGHRES_CONFIG_PATH}" >&2
  echo "[HINT] create it first (example): cp ${BASE_DIR}/highres.toml ${HIGHRES_CONFIG_PATH}" >&2
  exit 1
fi

exec "$START_SCRIPT" \
  --name "$NAME" \
  --ipc-prefix "$IPC_PREFIX" \
  --config "$CONFIG_PATH" \
  --highres-config "$HIGHRES_CONFIG_PATH" \
  "$@"
