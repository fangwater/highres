#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOL_SCRIPT="${SCRIPT_DIR}/deploy_stream_pairmm.sh"
PROFILE_A="okex-futures-binance-futures"
PROFILE_B="binance-margin-binance-futures"
PROFILE_C="binance-futures-binance-futures"

if [[ ! -f "$TOOL_SCRIPT" ]]; then
  echo "[ERROR] script not found: ${TOOL_SCRIPT}" >&2
  exit 1
fi

for arg in "$@"; do
  if [[ "$arg" == "--dir" ]]; then
    echo "[ERROR] --dir is not supported in deploy_all_target.sh" >&2
    echo "[HINT] use --base-dir to control target prefix" >&2
    exit 1
  fi
done

"$TOOL_SCRIPT" --profile "$PROFILE_A" "$@"
"$TOOL_SCRIPT" --profile "$PROFILE_B" "$@"
"$TOOL_SCRIPT" --profile "$PROFILE_C" "$@"
