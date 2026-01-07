#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  stop_stream_pairmm.sh [--name <pm2_name> | --ipc <path> | --all]

Examples:
  ./scripts/stop_stream_pairmm.sh --name stream_pairmm_sol
  ./scripts/stop_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc
  ./scripts/stop_stream_pairmm.sh --all
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
IPC_PATH=""
STOP_ALL="false"
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
    --ipc)
      IPC_PATH="${2:-}"
      if [[ -z "$IPC_PATH" ]]; then
        echo "[ERROR] --ipc requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --all)
      STOP_ALL="true"
      shift
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

if [[ "$STOP_ALL" == "true" ]]; then
  if ! command -v node >/dev/null 2>&1; then
    echo "[ERROR] node not found, cannot list pm2 processes for --all" >&2
    exit 1
  fi
  names="$(node -e '
    const {execSync} = require("child_process");
    const ns = process.argv[1];
    let list = [];
    try { list = JSON.parse(execSync("pm2 jlist", {encoding:"utf8"})); } catch { process.exit(1); }
    const names = list.filter(p => p.pm2_env && p.pm2_env.namespace === ns && p.name && p.name.startsWith("stream_pairmm"))
      .map(p => p.name);
    console.log(names.join("\n"));
  ' "$NAMESPACE")"
  if [[ -z "$names" ]]; then
    echo "[INFO] No stream_pairmm processes to stop (namespace: ${NAMESPACE})"
    exit 0
  fi
  while IFS= read -r name; do
    pm2 delete "$name" --namespace "$NAMESPACE" || true
  done <<< "$names"
  echo "[INFO] Stopped all stream_pairmm processes (namespace: ${NAMESPACE})"
  exit 0
fi

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
elif [[ -n "$IPC_PATH" ]]; then
  NAME="$(derive_name_from_ipc "$IPC_PATH")"
else
  echo "[ERROR] Need --name, --ipc, or --all" >&2
  usage >&2
  exit 1
fi

pm2 delete "$NAME" --namespace "$NAMESPACE" || true
echo "[INFO] Stopped: ${NAME} (namespace: ${NAMESPACE})"
