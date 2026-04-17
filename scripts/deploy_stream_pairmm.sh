#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STREAM_BIN_NAME="stream_pairmm"
RECORD_BIN_NAME="stream_pairmm_record"
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

is_one_exchange_profile() {
  [[ "$1" == "binance-futures-binance-futures" || "$1" == "okex-futures-okex-futures" ]]
}

resolve_first_existing() {
  local rel=""
  for rel in "$@"; do
    if [[ -f "$ROOT_DIR/$rel" ]]; then
      echo "$ROOT_DIR/$rel"
      return 0
    fi
  done
  return 1
}

resolve_stream_sources() {
  local profile="$1"
  local -n out_config="$2"
  local -n out_highres="$3"
  local -a config_candidates=()
  local -a highres_candidates=()

  if is_one_exchange_profile "$profile"; then
    config_candidates=("config.${profile}.toml" "config_one_exchange.toml" "config.toml")
    highres_candidates=("highres.${profile}.toml" "highres_one_exchange.toml" "highres.toml")
  else
    config_candidates=("config.${profile}.toml" "config_two_exchange.toml" "config.toml")
    highres_candidates=("highres.${profile}.toml" "highres_two_exchange.toml" "highres.toml")
  fi

  if ! out_config="$(resolve_first_existing "${config_candidates[@]}")"; then
    echo "[ERROR] no stream config found for profile=${profile}" >&2
    echo "[ERROR] tried: ${config_candidates[*]}" >&2
    return 1
  fi

  if ! out_highres="$(resolve_first_existing "${highres_candidates[@]}")"; then
    echo "[ERROR] no highres config found for profile=${profile}" >&2
    echo "[ERROR] tried: ${highres_candidates[*]}" >&2
    return 1
  fi
}

usage() {
  cat <<'USAGE'
Usage:
  deploy_stream_pairmm.sh --profile <name> [--host <user@ip>] [--base-dir <path_prefix>] [--dir <path>]
  deploy_stream_pairmm.sh --all [--host <user@ip>] [--base-dir <path_prefix>]

Defaults:
  --host      u171@10.61.10.32
  --base-dir  /home/u171/mth_pub
  --profile   one of:
              okex-futures-binance-futures
              okex-futures-okex-futures
              binance-margin-binance-futures
              binance-futures-binance-futures

Notes:
  --dir only works with single --profile mode.
  This script deploys stream + recorder stage:
  stream_pairmm, stream_pairmm_record, and related scripts.
USAGE
}

TARGET_HOST="u171@10.61.10.32"
BASE_TARGET_DIR="/home/u171/mth_pub"
PROFILE=""
DEPLOY_ALL="0"
TARGET_DIR_OVERRIDE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      TARGET_HOST="${2:-}"
      if [[ -z "$TARGET_HOST" ]]; then
        echo "[ERROR] --host requires user@ip" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --base-dir)
      BASE_TARGET_DIR="${2:-}"
      if [[ -z "$BASE_TARGET_DIR" ]]; then
        echo "[ERROR] --base-dir requires a path prefix" >&2
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
      DEPLOY_ALL="1"
      shift
      ;;
    --dir)
      TARGET_DIR_OVERRIDE="${2:-}"
      if [[ -z "$TARGET_DIR_OVERRIDE" ]]; then
        echo "[ERROR] --dir requires a path" >&2
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
      echo "[ERROR] unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$DEPLOY_ALL" == "1" ]] && [[ -n "$PROFILE" ]]; then
  echo "[ERROR] --all cannot be used with --profile" >&2
  exit 1
fi

if [[ "$DEPLOY_ALL" == "1" ]] && [[ -n "$TARGET_DIR_OVERRIDE" ]]; then
  echo "[ERROR] --dir is not supported with --all" >&2
  exit 1
fi

if [[ "$DEPLOY_ALL" != "1" ]] && [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required (or use --all)" >&2
  usage >&2
  exit 1
fi

if [[ -n "$PROFILE" ]] && ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

echo "[INFO] Building ${STREAM_BIN_NAME} and ${RECORD_BIN_NAME} (release)"
cargo build --release --bin "$STREAM_BIN_NAME" --bin "$RECORD_BIN_NAME"

STREAM_BIN_PATH="$ROOT_DIR/target/release/$STREAM_BIN_NAME"
if [[ ! -x "$STREAM_BIN_PATH" ]]; then
  echo "[ERROR] built stream binary not found: $STREAM_BIN_PATH" >&2
  exit 1
fi

RECORD_BIN_PATH="$ROOT_DIR/target/release/$RECORD_BIN_NAME"
if [[ ! -x "$RECORD_BIN_PATH" ]]; then
  echo "[ERROR] built recorder binary not found: $RECORD_BIN_PATH" >&2
  exit 1
fi

deploy_one() {
  local profile="$1"
  local target_dir=""

  if [[ -n "$TARGET_DIR_OVERRIDE" ]]; then
    target_dir="$TARGET_DIR_OVERRIDE"
  else
    target_dir="${BASE_TARGET_DIR}-${profile}"
  fi

  local config_src=""
  local highres_src=""
  resolve_stream_sources "$profile" config_src highres_src

  echo "[INFO] Deploy stream profile=${profile} -> ${TARGET_HOST}:${target_dir}"
  echo "[INFO] stream config=$(basename "$config_src"), highres=$(basename "$highres_src")"

  ssh "$TARGET_HOST" "mkdir -p \"$target_dir/scripts\""

  rsync -a "$STREAM_BIN_PATH" "$TARGET_HOST:$target_dir/"
  rsync -a "$RECORD_BIN_PATH" "$TARGET_HOST:$target_dir/"
  ssh "$TARGET_HOST" "chmod +x \"$target_dir/$STREAM_BIN_NAME\" \"$target_dir/$RECORD_BIN_NAME\""

  rsync -a "$config_src" "$TARGET_HOST:$target_dir/config.toml"
  rsync -a "$highres_src" "$TARGET_HOST:$target_dir/highres.toml"
  rsync -a "$config_src" "$TARGET_HOST:$target_dir/config.${profile}.toml"
  rsync -a "$highres_src" "$TARGET_HOST:$target_dir/highres.${profile}.toml"

  if [[ -f "$ROOT_DIR/log4rs.yaml" ]]; then
    rsync -a "$ROOT_DIR/log4rs.yaml" "$TARGET_HOST:$target_dir/"
  fi

  for cache in "$ROOT_DIR"/*_cache.json; do
    if [[ -f "$cache" ]]; then
      rsync -a "$cache" "$TARGET_HOST:$target_dir/"
    fi
  done

  for script in \
    start_stream_pairmm_batch.sh \
    start_stream_pairmm_record.sh \
    stop_stream_pairmm_batch.sh \
    stop_stream_pairmm_record.sh \
    export_stream_pairmm_record.sh \
    stream_pairmm_batch_runner.sh; do
    if [[ -f "$ROOT_DIR/scripts/$script" ]]; then
      rsync -a "$ROOT_DIR/scripts/$script" "$TARGET_HOST:$target_dir/scripts/"
      ssh "$TARGET_HOST" "chmod +x \"$target_dir/scripts/$script\""
    fi
  done

  echo "[INFO] deployed stream profile=${profile} to ${TARGET_HOST}:${target_dir}"
  echo "[INFO] start: ssh $TARGET_HOST \"cd $target_dir && ./scripts/start_stream_pairmm_batch.sh --profile $profile\""
  echo "[INFO] rec  : ssh $TARGET_HOST \"cd $target_dir && ./scripts/start_stream_pairmm_record.sh --profile $profile\""
  echo "[INFO] stop : ssh $TARGET_HOST \"cd $target_dir && ./scripts/stop_stream_pairmm_batch.sh --profile $profile\""
  echo "[INFO] rec  : ssh $TARGET_HOST \"cd $target_dir && ./scripts/stop_stream_pairmm_record.sh --profile $profile\""
}

if [[ "$DEPLOY_ALL" == "1" ]]; then
  for p in "${SUPPORTED_PROFILES[@]}"; do
    deploy_one "$p"
  done
  echo "[INFO] deployed all stream profiles"
else
  deploy_one "$PROFILE"
fi
