#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAMES=(
  "stream_pairmm"
  "stream_pairmm_record"
  "pnlu_factor_stream"
)
SUPPORTED_PROFILES=("okex-futures-binance-futures" "binance-futures-binance-futures")

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

  case "$profile" in
    okex-futures-binance-futures)
      config_candidates=("config.${profile}.toml" "config_two_exchange.toml" "config.toml")
      highres_candidates=("highres.${profile}.toml" "highres_two_exchange.toml" "highres.toml")
      ;;
    binance-futures-binance-futures)
      config_candidates=("config.${profile}.toml" "config_one_exchange.toml" "config.toml")
      highres_candidates=("highres.${profile}.toml" "highres_one_exchange.toml" "highres.toml")
      ;;
    *)
      config_candidates=("config.${profile}.toml" "config.toml")
      highres_candidates=("highres.${profile}.toml" "highres.toml")
      ;;
  esac

  if ! out_config="$(resolve_first_existing "${config_candidates[@]}")"; then
    echo "[ERROR] no config file found for profile=${profile}" >&2
    echo "[ERROR] tried: ${config_candidates[*]}" >&2
    return 1
  fi

  if ! out_highres="$(resolve_first_existing "${highres_candidates[@]}")"; then
    echo "[ERROR] no highres file found for profile=${profile}" >&2
    echo "[ERROR] tried: ${highres_candidates[*]}" >&2
    return 1
  fi
}

resolve_pnlu_sources() {
  local profile="$1"
  local -n out_stream_conf="$2"
  local -n out_rolling_conf="$3"
  local -n out_rolling_symbols="$4"
  local -a stream_candidates=()
  local -a rolling_candidates=()
  local -a rolling_symbols_candidates=()

  stream_candidates=("pnlu_factor.${profile}.toml" "pnlu_factor.toml")
  rolling_candidates=("pnlu_factor_rolling.${profile}.toml" "pnlu_factor_rolling.toml")
  rolling_symbols_candidates=("pnlu_factor_rolling_symbols.${profile}.json" "pnlu_factor_rolling_symbols.json")

  if ! out_stream_conf="$(resolve_first_existing "${stream_candidates[@]}")"; then
    echo "[ERROR] no pnlu_factor config found for profile=${profile}" >&2
    echo "[ERROR] tried: ${stream_candidates[*]}" >&2
    return 1
  fi

  if ! out_rolling_conf="$(resolve_first_existing "${rolling_candidates[@]}")"; then
    echo "[ERROR] no pnlu_factor_rolling config found for profile=${profile}" >&2
    echo "[ERROR] tried: ${rolling_candidates[*]}" >&2
    return 1
  fi

  if ! out_rolling_symbols="$(resolve_first_existing "${rolling_symbols_candidates[@]}")"; then
    echo "[ERROR] no pnlu_factor_rolling_symbols file found for profile=${profile}" >&2
    echo "[ERROR] tried: ${rolling_symbols_candidates[*]}" >&2
    return 1
  fi
}

usage() {
  cat <<'EOF'
Usage:
  deploy_stream_pairmm.sh --profile <name> [--host <user@ip>] [--base-dir <path_prefix>] [--dir <path>]

Defaults:
  --host      u171@10.61.10.32
  --base-dir  /home/u171/mth_pub
  --profile   required, values:
              okex-futures-binance-futures
              binance-futures-binance-futures
  --dir       optional, override final target dir for this profile only

Examples:
  bash scripts/deploy_stream_pairmm.sh --profile okex-futures-binance-futures
  bash scripts/deploy_stream_pairmm.sh --profile binance-futures-binance-futures
  bash scripts/deploy_stream_pairmm.sh --profile okex-futures-binance-futures --base-dir "/home/u171/mth_pub"
  bash scripts/deploy_stream_pairmm.sh --profile binance-futures-binance-futures --dir "/home/u171/mth_pub-binance-futures-binance-futures"

This script deploys all runtime components together:
  stream_pairmm + stream_pairmm_record + pnlu_factor_stream (includes rolling runtime)
EOF
}

TARGET_HOST="u171@10.61.10.32"
BASE_TARGET_DIR="/home/u171/mth_pub"
PROFILE=""
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
      echo "[ERROR] Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required" >&2
  usage >&2
  exit 1
fi

if [[ "$PROFILE" == *","* ]]; then
  echo "[ERROR] --profile only accepts one value" >&2
  exit 1
fi

if [[ "$PROFILE" == "all-target" ]]; then
  echo "[ERROR] --profile all-target is not supported" >&2
  echo "[HINT] use scripts/deploy_all_target.sh" >&2
  exit 1
fi

is_supported="0"
for p in "${SUPPORTED_PROFILES[@]}"; do
  if [[ "$PROFILE" == "$p" ]]; then
    is_supported="1"
    break
  fi
done

if [[ "$is_supported" != "1" ]]; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

for BIN_NAME in "${BIN_NAMES[@]}"; do
  echo "[INFO] Building $BIN_NAME (release)"
  cargo build --release --bin "$BIN_NAME"
done

if [[ -n "$TARGET_DIR_OVERRIDE" ]]; then
  TARGET_DIR="$TARGET_DIR_OVERRIDE"
else
  TARGET_DIR="${BASE_TARGET_DIR}-${PROFILE}"
fi

CONFIG_SRC=""
HIGHRES_SRC=""
resolve_stream_sources "$PROFILE" CONFIG_SRC HIGHRES_SRC
PNLU_STREAM_CONF=""
PNLU_ROLLING_CONF=""
PNLU_ROLLING_SYMBOLS=""
resolve_pnlu_sources "$PROFILE" PNLU_STREAM_CONF PNLU_ROLLING_CONF PNLU_ROLLING_SYMBOLS

echo "[INFO] Deploy profile=${PROFILE} -> ${TARGET_HOST}:${TARGET_DIR}"
echo "[INFO] use stream config=$(basename "$CONFIG_SRC"), highres=$(basename "$HIGHRES_SRC")"
echo "[INFO] use pnlu config=$(basename "$PNLU_STREAM_CONF"), rolling=$(basename "$PNLU_ROLLING_CONF"), rolling_symbols=$(basename "$PNLU_ROLLING_SYMBOLS")"

ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR\""
for BIN_NAME in "${BIN_NAMES[@]}"; do
  BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"
  rsync -a "$BIN_PATH" "$TARGET_HOST:$TARGET_DIR/"
  ssh "$TARGET_HOST" "chmod +x \"$TARGET_DIR/$BIN_NAME\""
done

rsync -a "$CONFIG_SRC" "$TARGET_HOST:$TARGET_DIR/config.toml"
rsync -a "$HIGHRES_SRC" "$TARGET_HOST:$TARGET_DIR/highres.toml"
rsync -a "$CONFIG_SRC" "$TARGET_HOST:$TARGET_DIR/config.${PROFILE}.toml"
rsync -a "$HIGHRES_SRC" "$TARGET_HOST:$TARGET_DIR/highres.${PROFILE}.toml"
rsync -a "$PNLU_STREAM_CONF" "$TARGET_HOST:$TARGET_DIR/pnlu_factor.toml"
rsync -a "$PNLU_ROLLING_CONF" "$TARGET_HOST:$TARGET_DIR/pnlu_factor_rolling.toml"
rsync -a "$PNLU_ROLLING_SYMBOLS" "$TARGET_HOST:$TARGET_DIR/pnlu_factor_rolling_symbols.json"
rsync -a "$PNLU_STREAM_CONF" "$TARGET_HOST:$TARGET_DIR/pnlu_factor.${PROFILE}.toml"
rsync -a "$PNLU_ROLLING_CONF" "$TARGET_HOST:$TARGET_DIR/pnlu_factor_rolling.${PROFILE}.toml"
rsync -a "$PNLU_ROLLING_SYMBOLS" "$TARGET_HOST:$TARGET_DIR/pnlu_factor_rolling_symbols.${PROFILE}.json"

if [[ -f "$ROOT_DIR/log4rs.yaml" ]]; then
  rsync -a "$ROOT_DIR/log4rs.yaml" "$TARGET_HOST:$TARGET_DIR/"
fi

for cache in "$ROOT_DIR"/*_cache.json; do
  if [[ -f "$cache" ]]; then
    rsync -a "$cache" "$TARGET_HOST:$TARGET_DIR/"
  fi
done

ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR/logs\""

for script in \
  start_stream_pairmm.sh \
  start_stream_pairmm_batch.sh \
  start_stream_pairmm_batch_two_exchange.sh \
  start_stream_pairmm_batch_one_exchange.sh \
  start_pnlu_factor_stream.sh \
  stop_pnlu_factor_stream.sh \
  deploy_all_target.sh \
  stream_pairmm_batch_runner.sh \
  stop_stream_pairmm_batch.sh \
  stop_stream_pairmm_batch_two_exchange.sh \
  stop_stream_pairmm_batch_one_exchange.sh \
  start_stream_pairmm_record.sh \
  stop_stream_pairmm_record.sh \
  export_stream_pairmm_record.sh; do
  if [[ -f "$ROOT_DIR/scripts/$script" ]]; then
    ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR/scripts\""
    rsync -a "$ROOT_DIR/scripts/$script" "$TARGET_HOST:$TARGET_DIR/scripts/"
    ssh "$TARGET_HOST" "chmod +x \"$TARGET_DIR/scripts/$script\""
  fi
done

echo "[INFO] deployed profile=${PROFILE} to ${TARGET_HOST}:${TARGET_DIR}"
echo "[INFO] start example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_stream_pairmm_batch.sh --name stream_pairmm_batch-$PROFILE --ipc-prefix /tmp/mth_pubs/$PROFILE --config ./config.toml --highres-config ./highres.toml\""
echo "[INFO] record example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_stream_pairmm_record.sh --profile $PROFILE\""
echo "[INFO] pnlu stream example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_pnlu_factor_stream.sh --name pnlu_factor_stream-$PROFILE --profile $PROFILE --ipc-prefix /tmp/mth_pubs/stream_pairmm/$PROFILE --config ./pnlu_factor.toml\""
echo "[INFO] note: rolling is embedded in pnlu_factor_stream, config file is ./pnlu_factor_rolling.toml"
echo "[INFO] deploy finished. profile=${PROFILE}"
