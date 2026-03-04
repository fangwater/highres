#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SUPPORTED_PROFILES=(
  "okex-futures-binance-futures"
  "binance-margin-binance-futures"
  "binance-futures-binance-futures"
)

is_supported_profile() {
  local profile="$1"
  local p
  for p in "${SUPPORTED_PROFILES[@]}"; do
    if [[ "$p" == "$profile" ]]; then
      return 0
    fi
  done
  return 1
}

usage() {
  cat <<'EOF'
Usage:
  stop_stream_pairmm_record.sh [--name <pm2_name>] [--profile <name>] [--all]

Examples:
  ./scripts/stop_stream_pairmm_record.sh
  ./scripts/stop_stream_pairmm_record.sh --profile okex-futures-binance-futures
  ./scripts/stop_stream_pairmm_record.sh --profile binance-margin-binance-futures
  ./scripts/stop_stream_pairmm_record.sh --profile binance-futures-binance-futures
  ./scripts/stop_stream_pairmm_record.sh --all
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
PROFILE=""
STOP_ALL="0"
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
    --all)
      STOP_ALL="1"
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

if [[ "$STOP_ALL" == "1" ]] && ([[ -n "$NAME_OVERRIDE" ]] || [[ -n "$PROFILE" ]]); then
  echo "[ERROR] --all cannot be used with --name/--profile" >&2
  exit 1
fi

if [[ "$STOP_ALL" == "1" ]]; then
  for p in "${SUPPORTED_PROFILES[@]}"; do
    "$0" --profile "$p"
  done
  exit 0
fi

if [[ -n "$PROFILE" ]] && ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
elif [[ -n "$PROFILE" ]]; then
  NAME="stream_pairmm_record-${PROFILE}"
else
  NAME="stream_pairmm_record"
fi
NAMESPACE="$(basename "${BASE_DIR}")"

pm2 delete "$NAME" --namespace "$NAMESPACE" || true
echo "[INFO] Stopped: ${NAME} (namespace: ${NAMESPACE})"
