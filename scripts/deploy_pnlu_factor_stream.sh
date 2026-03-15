#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="pnlu_factor_stream"
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

resolve_pnlu_sources() {
  local profile="$1"
  local -n out_config="$2"
  local -n out_symbols="$3"

  out_config="$ROOT_DIR/pnlu_factor.${profile}.toml"
  if [[ ! -f "$out_config" ]]; then
    echo "[ERROR] no pnlu config found for profile=${profile}" >&2
    return 1
  fi

  out_symbols="$ROOT_DIR/pnlu_factor_rolling_symbols.${profile}.json"
  if [[ ! -f "$out_symbols" ]]; then
    echo "[ERROR] no pnlu rolling symbols config found for profile=${profile}" >&2
    return 1
  fi
}

usage() {
  cat <<'USAGE'
Usage:
  deploy_pnlu_factor_stream.sh --profile <name> [--host <user@ip>] [--base-dir <path_prefix>]
  deploy_pnlu_factor_stream.sh --all [--host <user@ip>] [--base-dir <path_prefix>]

Defaults:
  --host      u171@10.61.10.32
  --base-dir  /home/u171/mth_pub

Notes:
  target dir is fixed to <base-dir>-<profile>, exactly the same as stream_pairmm deploy dirs
  remote config.toml must already exist in that stream_pairmm dir
USAGE
}

TARGET_HOST="u171@10.61.10.32"
BASE_TARGET_DIR="/home/u171/mth_pub"
PROFILE=""
DEPLOY_ALL="0"

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

echo "[INFO] Building ${BIN_NAME} (release)"
cargo build --release --bin "$BIN_NAME"

BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"
if [[ ! -x "$BIN_PATH" ]]; then
  echo "[ERROR] built binary not found: $BIN_PATH" >&2
  exit 1
fi

deploy_one() {
  local profile="$1"
  local target_dir=""
  local pnlu_config_src=""
  local symbols_src=""
  target_dir="${BASE_TARGET_DIR}-${profile}"

  resolve_pnlu_sources "$profile" pnlu_config_src symbols_src

  echo "[INFO] Deploy pnlu profile=${profile} -> ${TARGET_HOST}:${target_dir}"
  echo "[INFO] pnlu config=$(basename "$pnlu_config_src"), rolling symbols=$(basename "$symbols_src")"

  ssh "$TARGET_HOST" "test -f \"$target_dir/config.toml\" && mkdir -p \"$target_dir/scripts\""

  rsync -a "$BIN_PATH" "$TARGET_HOST:$target_dir/"
  ssh "$TARGET_HOST" "chmod +x \"$target_dir/$BIN_NAME\""

  rsync -a "$pnlu_config_src" "$TARGET_HOST:$target_dir/pnlu_factor.toml"

  rsync -a "$symbols_src" "$TARGET_HOST:$target_dir/pnlu_factor_rolling_symbols.json"

  for script in \
    start_pnlu_factor_stream.sh \
    stop_pnlu_factor_stream.sh \
    print_pnlu_factor_thresholds.py \
    print_pnlu_factor_ipc.py; do
    if [[ -f "$ROOT_DIR/scripts/$script" ]]; then
      rsync -a "$ROOT_DIR/scripts/$script" "$TARGET_HOST:$target_dir/scripts/"
      ssh "$TARGET_HOST" "chmod +x \"$target_dir/scripts/$script\""
    fi
  done

  echo "[INFO] deployed pnlu profile=${profile} to ${TARGET_HOST}:${target_dir}"
  echo "[INFO] start: ssh $TARGET_HOST \"cd $target_dir && ./scripts/start_pnlu_factor_stream.sh --profile $profile\""
  echo "[INFO] stop : ssh $TARGET_HOST \"cd $target_dir && ./scripts/stop_pnlu_factor_stream.sh --profile $profile\""
}

if [[ "$DEPLOY_ALL" == "1" ]]; then
  for p in "${SUPPORTED_PROFILES[@]}"; do
    deploy_one "$p"
  done
  echo "[INFO] deployed all pnlu profiles"
else
  deploy_one "$PROFILE"
fi
