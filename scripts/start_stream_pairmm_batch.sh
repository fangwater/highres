#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm_batch.sh [--ipc-prefix <path>]

Defaults:
  --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures

Examples:
  ./scripts/start_stream_pairmm_batch.sh
  ./scripts/start_stream_pairmm_batch.sh --ipc-prefix /tmp/mth_pubs/okex-futures-binance-futures
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

IPC_PREFIX="/tmp/mth_pubs/okex-futures-binance-futures"
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

CONFIG_PATH="${BASE_DIR}/config.toml"
if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] config.toml not found in ${BASE_DIR}" >&2
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
    import tomllib as toml  # py3.11+
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
  echo "[ERROR] online_symbols is empty in config.toml" >&2
  exit 1
fi

for symbol in $symbols_raw; do
  ipc_path="${IPC_PREFIX}/${symbol}.ipc"
  echo "[INFO] start ${symbol} ipc=${ipc_path}"
  "${SCRIPT_DIR}/start_stream_pairmm.sh" --ipc "$ipc_path"
done
