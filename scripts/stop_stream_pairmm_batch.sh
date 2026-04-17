#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

SUPPORTED_PROFILES=(
  "okex-futures-binance-futures"
  "okex-futures-okex-futures"
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

infer_profile_from_base_dir() {
  local base_name=""
  base_name="$(basename "$BASE_DIR")"

  if is_supported_profile "$base_name"; then
    echo "$base_name"
    return 0
  fi

  local p=""
  for p in "${SUPPORTED_PROFILES[@]}"; do
    if [[ "$base_name" == *"$p" ]]; then
      echo "$p"
      return 0
    fi
  done

  return 1
}

profile_alias() {
  case "$1" in
    okex-futures-binance-futures) echo "ok-futures-bn-futures" ;;
    okex-futures-okex-futures) echo "ok-futures-ok-futures" ;;
    binance-margin-binance-futures) echo "bn-margin-bn-futures" ;;
    binance-futures-binance-futures) echo "bn-futures-bn-futures" ;;
    *) echo "$1" ;;
  esac
}

usage() {
  cat <<'EOF'
Usage:
  stop_stream_pairmm_batch.sh [--name <pm2_name>] [--profile <name>] [--all]

Examples:
  ./scripts/stop_stream_pairmm_batch.sh
  ./scripts/stop_stream_pairmm_batch.sh --profile okex-futures-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --profile okex-futures-okex-futures
  ./scripts/stop_stream_pairmm_batch.sh --profile binance-margin-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --profile binance-futures-binance-futures
  ./scripts/stop_stream_pairmm_batch.sh --all
  ./scripts/stop_stream_pairmm_batch.sh --name stream_pairmm_batch
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

if [[ -z "$PROFILE" ]] && [[ -z "$NAME_OVERRIDE" ]]; then
  PROFILE="$(infer_profile_from_base_dir || true)"
fi

if [[ -n "$PROFILE" ]] && ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
elif [[ -n "$PROFILE" ]]; then
  NAME="stream_pairmm_batch-$(profile_alias "$PROFILE")"
else
  NAME="stream_pairmm_batch"
fi
NAMESPACE="$(basename "${BASE_DIR}")"

pm2 delete "$NAME" --namespace "$NAMESPACE" || true
if [[ -n "$PROFILE" ]]; then
  pm2 delete "stream_pairmm_batch-${PROFILE}" --namespace "$NAMESPACE" >/dev/null 2>&1 || true
fi
STOP_RECORD_SCRIPT="${SCRIPT_DIR}/stop_stream_pairmm_record.sh"
RECORD_NAME=""
if [[ -f "$STOP_RECORD_SCRIPT" ]]; then
  if [[ -n "$PROFILE" ]]; then
    RECORD_NAME="stream_pairmm_record-${PROFILE}"
    "$STOP_RECORD_SCRIPT" --profile "$PROFILE"
  else
    RECORD_NAME="${NAME}-record"
    "$STOP_RECORD_SCRIPT" --name "$RECORD_NAME"
  fi
else
  RECORD_NAME="${NAME}-record"
  pm2 delete "$RECORD_NAME" --namespace "$NAMESPACE" || true
fi

echo "[INFO] Stopped: ${NAME} + ${RECORD_NAME} (namespace: ${NAMESPACE})"
