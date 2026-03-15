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

is_one_exchange_profile() {
  [[ "$1" == "binance-futures-binance-futures" ]]
}

profile_alias() {
  case "$1" in
    okex-futures-binance-futures) echo "ok-futures-bn-futures" ;;
    binance-margin-binance-futures) echo "bn-margin-bn-futures" ;;
    binance-futures-binance-futures) echo "bn-futures-bn-futures" ;;
    *) echo "$1" ;;
  esac
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

default_stream_config_path() {
  local profile="$1"
  local profile_path="${BASE_DIR}/config.${profile}.toml"
  if [[ -f "$profile_path" ]]; then
    echo "$profile_path"
    return
  fi
  if is_one_exchange_profile "$profile"; then
    echo "${BASE_DIR}/config_one_exchange.toml"
  else
    echo "${BASE_DIR}/config_two_exchange.toml"
  fi
}

default_highres_config_path() {
  local profile="$1"
  local profile_path="${BASE_DIR}/highres.${profile}.toml"
  if [[ -f "$profile_path" ]]; then
    echo "$profile_path"
    return
  fi
  if is_one_exchange_profile "$profile"; then
    echo "${BASE_DIR}/highres_one_exchange.toml"
  else
    echo "${BASE_DIR}/highres_two_exchange.toml"
  fi
}

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm_batch.sh [--profile <name>] [--ipc-prefix <path>] [--name <pm2_name>] [--config <path>] [--highres-config <path>] [--bin <path>]
  start_stream_pairmm_batch.sh --all [--bin <path>]

Defaults:
  --profile    one of:
               okex-futures-binance-futures          (two-exchange)
               binance-margin-binance-futures        (two-exchange)
               binance-futures-binance-futures       (one-exchange)
               if omitted, infer from current deploy dir name when possible
  --ipc-prefix /tmp/mth_pubs/<profile>
  --name       stream_pairmm_batch-<profile>
  --config     <repo>/config.<profile>.toml (if exists)
               fallback: two-exchange -> config_two_exchange.toml
                         one-exchange -> config_one_exchange.toml
  --highres-config <repo>/highres.<profile>.toml (if exists)
               fallback: two-exchange -> highres_two_exchange.toml
                         one-exchange -> highres_one_exchange.toml
  子进程日志固定写到 /mnt/data/stream_pairmm/<profile>/<symbol>.log

Examples:
  ./scripts/start_stream_pairmm_batch.sh --profile okex-futures-binance-futures
  ./scripts/start_stream_pairmm_batch.sh --profile binance-margin-binance-futures
  ./scripts/start_stream_pairmm_batch.sh --profile binance-futures-binance-futures
  ./scripts/start_stream_pairmm_batch.sh --all
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

PROFILE=""
IPC_PREFIX=""
NAME=""
IPC_PREFIX_SET="0"
NAME_SET="0"
CONFIG_PATH=""
CONFIG_SET="0"
HIGHRES_CONFIG_PATH=""
HIGHRES_SET="0"
BIN_OVERRIDE=""
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
    --ipc-prefix)
      IPC_PREFIX="${2:-}"
      if [[ -z "$IPC_PREFIX" ]]; then
        echo "[ERROR] --ipc-prefix requires a value" >&2
        usage >&2
        exit 1
      fi
      IPC_PREFIX_SET="1"
      shift 2
      ;;
    --name)
      NAME="${2:-}"
      if [[ -z "$NAME" ]]; then
        echo "[ERROR] --name requires a value" >&2
        usage >&2
        exit 1
      fi
      NAME_SET="1"
      shift 2
      ;;
    --config)
      CONFIG_PATH="${2:-}"
      if [[ -z "$CONFIG_PATH" ]]; then
        echo "[ERROR] --config requires a value" >&2
        usage >&2
        exit 1
      fi
      CONFIG_SET="1"
      shift 2
      ;;
    --highres-config)
      HIGHRES_CONFIG_PATH="${2:-}"
      if [[ -z "$HIGHRES_CONFIG_PATH" ]]; then
        echo "[ERROR] --highres-config requires a value" >&2
        usage >&2
        exit 1
      fi
      HIGHRES_SET="1"
      shift 2
      ;;
    --bin)
      BIN_OVERRIDE="${2:-}"
      if [[ -z "$BIN_OVERRIDE" ]]; then
        echo "[ERROR] --bin requires a value" >&2
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
  if [[ "$IPC_PREFIX_SET" == "1" ]] || [[ "$NAME_SET" == "1" ]] || [[ "$CONFIG_SET" == "1" ]] || [[ "$HIGHRES_SET" == "1" ]]; then
    echo "[ERROR] --all does not accept --ipc-prefix/--name/--config/--highres-config overrides" >&2
    exit 1
  fi
  COMMON_ARGS=()
  if [[ -n "$BIN_OVERRIDE" ]]; then
    COMMON_ARGS+=(--bin "$BIN_OVERRIDE")
  fi
  for p in "${SUPPORTED_PROFILES[@]}"; do
    echo "[INFO] start stream profile=${p}"
    bash "$0" --profile "$p" "${COMMON_ARGS[@]}"
  done
  exit 0
fi

if [[ -z "$PROFILE" ]]; then
  PROFILE="$(infer_profile_from_base_dir || true)"
fi

if [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required and could not be inferred from current dir" >&2
  usage >&2
  exit 1
fi

if ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

if [[ "$IPC_PREFIX_SET" == "0" ]]; then
  IPC_PREFIX="/tmp/mth_pubs/${PROFILE}"
fi
if [[ "$NAME_SET" == "0" ]]; then
  NAME="stream_pairmm_batch-$(profile_alias "$PROFILE")"
fi
if [[ "$CONFIG_SET" == "0" ]]; then
  CONFIG_PATH="$(default_stream_config_path "$PROFILE")"
fi
if [[ "$HIGHRES_SET" == "0" ]]; then
  HIGHRES_CONFIG_PATH="$(default_highres_config_path "$PROFILE")"
fi

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "[ERROR] config file not found: ${CONFIG_PATH}" >&2
  exit 1
fi

if [[ ! -f "$HIGHRES_CONFIG_PATH" ]]; then
  echo "[ERROR] highres config file not found: ${HIGHRES_CONFIG_PATH}" >&2
  exit 1
fi

RUNNER_PATH="${SCRIPT_DIR}/stream_pairmm_batch_runner.sh"
if [[ ! -f "$RUNNER_PATH" ]]; then
  echo "[ERROR] runner script not found: ${RUNNER_PATH}" >&2
  exit 1
fi

NAMESPACE="$(basename "${BASE_DIR}")"

echo "[INFO] Restarting ${NAME}"
pm2 delete "stream_pairmm_batch-${PROFILE}" --namespace "$NAMESPACE" >/dev/null 2>&1 || true
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

PM2_CMD=(
  pm2 start "$RUNNER_PATH"
  --interpreter bash
  --name "$NAME"
  --namespace "$NAMESPACE"
  --cwd "$BASE_DIR"
  --
  --ipc-prefix "$IPC_PREFIX"
  --config "$CONFIG_PATH"
  --highres-config "$HIGHRES_CONFIG_PATH"
)

if [[ -n "$BIN_OVERRIDE" ]]; then
  PM2_CMD+=(--bin "$BIN_OVERRIDE")
fi

RUST_LOG="${RUST_LOG:-info}" "${PM2_CMD[@]}"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Profile: ${PROFILE}"
if is_one_exchange_profile "$PROFILE"; then
  echo "Mode: one-exchange"
else
  echo "Mode: two-exchange"
fi
echo "IPC: ${IPC_PREFIX}"
echo "Config: ${CONFIG_PATH}"
echo "Highres: ${HIGHRES_CONFIG_PATH}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
