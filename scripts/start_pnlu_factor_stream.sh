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

profile_alias() {
  case "$1" in
    okex-futures-binance-futures) echo "ok-futures-bn-futures" ;;
    okex-futures-okex-futures) echo "ok-futures-ok-futures" ;;
    binance-margin-binance-futures) echo "bn-margin-bn-futures" ;;
    binance-futures-binance-futures) echo "bn-futures-bn-futures" ;;
    *) echo "$1" ;;
  esac
}

normalize_profile_channel() {
  local raw="$1"
  local token="$raw"
  token="${token##*/}"
  token="${token%.toml}"
  token="${token,,}"
  token="${token//_/-}"
  if [[ "$token" =~ ^[0-9]+-(.+)$ ]]; then
    token="${BASH_REMATCH[1]}"
  fi
  token="${token#-}"
  token="${token%-}"
  echo "$token"
}

usage() {
  cat <<'EOF'
Usage:
  start_pnlu_factor_stream.sh --profile <name>
  start_pnlu_factor_stream.sh --all

Examples:
  ./scripts/start_pnlu_factor_stream.sh --profile okex-futures-binance-futures
  ./scripts/start_pnlu_factor_stream.sh --profile okex-futures-okex-futures
  ./scripts/start_pnlu_factor_stream.sh --profile binance-margin-binance-futures
  ./scripts/start_pnlu_factor_stream.sh --profile binance-futures-binance-futures
  ./scripts/start_pnlu_factor_stream.sh --all
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

PROFILE=""
START_ALL="0"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      START_ALL="1"
      shift
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

if [[ "$START_ALL" == "1" ]]; then
  if [[ -n "$PROFILE" ]]; then
    echo "[ERROR] --all cannot be used with --profile" >&2
    exit 1
  fi
  for p in "${SUPPORTED_PROFILES[@]}"; do
    echo "[INFO] start pnlu factor profile=${p}"
    bash "$0" --profile "$p"
  done
  exit 0
fi

PROFILE_CHANNEL=""
if [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required (or use --all)" >&2
  usage >&2
  exit 1
fi

PROFILE_CHANNEL="$(normalize_profile_channel "$PROFILE")"
if [[ -z "$PROFILE_CHANNEL" ]]; then
  echo "[ERROR] invalid --profile: ${PROFILE}" >&2
  exit 1
fi
if ! is_supported_profile "$PROFILE_CHANNEL"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

CONFIG_PATH="${BASE_DIR}/pnlu_factor.toml"
if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] pnlu_factor.toml not found in ${BASE_DIR}" >&2
  exit 1
fi
if [[ ! -f "${BASE_DIR}/config.toml" ]]; then
  echo "[ERROR] config.toml not found in ${BASE_DIR}" >&2
  exit 1
fi

OUTPUT_IPC_PREFIX="ipc:///tmp/mth_pubs/pnlu_factor/${PROFILE_CHANNEL}.ipc"
IPC_PREFIX="/tmp/mth_pubs/stream_pairmm/${PROFILE_CHANNEL}"
NAME="pnlu_factor_stream-$(profile_alias "$PROFILE_CHANNEL")"
NAMESPACE="$(basename "${BASE_DIR}")"

BIN_CANDIDATES=(
  "${BASE_DIR}/pnlu_factor_stream"
  "${BASE_DIR}/target/release/pnlu_factor_stream"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -f "$cand" && -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] pnlu_factor_stream binary not found. Build first with: cargo build --release --bin pnlu_factor_stream" >&2
  exit 1
fi

ARGS=()
if [[ -n "$IPC_PREFIX" ]]; then
  ARGS+=(--ipc-prefix "$IPC_PREFIX")
fi
if [[ -n "$OUTPUT_IPC_PREFIX" ]]; then
  ARGS+=(--output-ipc-prefix "$OUTPUT_IPC_PREFIX")
fi
if [[ -n "$PROFILE" ]]; then
  ARGS+=(--profile "$PROFILE")
fi
ARGS+=(--config "$CONFIG_PATH")

echo "[INFO] Restarting ${NAME}"
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

PM2_CMD=(pm2 start "$BIN_PATH" --name "$NAME" --namespace "$NAMESPACE" --cwd "$BASE_DIR")
if [[ ${#ARGS[@]} -gt 0 ]]; then
  PM2_CMD+=(-- "${ARGS[@]}")
fi

RUST_LOG="${RUST_LOG:-info}" "${PM2_CMD[@]}"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Profile: ${PROFILE_CHANNEL}"
echo "Config: ${CONFIG_PATH}"
echo "Input IPC: ${IPC_PREFIX}"
echo "Output IPC: ${OUTPUT_IPC_PREFIX}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
