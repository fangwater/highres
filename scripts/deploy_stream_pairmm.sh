#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAMES=("stream_pairmm" "stream_pairmm_record")

usage() {
  cat <<'EOF'
Usage:
  deploy_stream_pairmm.sh [--host <user@ip>] [--dir <path>]

Defaults:
  --host u171@10.61.10.32
  --dir /home/u171/mth_pub

Examples:
  bash scripts/deploy_stream_pairmm.sh
  bash scripts/deploy_stream_pairmm.sh --host u171@10.61.10.32
  bash scripts/deploy_stream_pairmm.sh --dir "/home/u171/mth_pub"
EOF
}

TARGET_HOST=""
TARGET_DIR=""
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
    --dir)
      TARGET_DIR="${2:-}"
      if [[ -z "$TARGET_DIR" ]]; then
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

if [[ -z "$TARGET_HOST" ]]; then
  TARGET_HOST="u171@10.61.10.32"
fi

if [[ -z "$TARGET_DIR" ]]; then
  TARGET_DIR="/home/u171/mth_pub"
fi

for BIN_NAME in "${BIN_NAMES[@]}"; do
  echo "[INFO] Building $BIN_NAME (release)"
  cargo build --release --bin "$BIN_NAME"
done

echo "[INFO] Deploying binaries to $TARGET_HOST:$TARGET_DIR"
ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR\""
for BIN_NAME in "${BIN_NAMES[@]}"; do
  BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"
  rsync -a "$BIN_PATH" "$TARGET_HOST:$TARGET_DIR/"
  ssh "$TARGET_HOST" "chmod +x \"$TARGET_DIR/$BIN_NAME\""
done

for conf in highres.toml log4rs.yaml; do
  if [[ -f "$ROOT_DIR/$conf" ]]; then
    rsync -a "$ROOT_DIR/$conf" "$TARGET_HOST:$TARGET_DIR/"
  fi
done

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

echo "[INFO] Binaries deployed to $TARGET_HOST:$TARGET_DIR"
echo "[INFO] Example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_stream_pairmm_record.sh\""
echo "[INFO] Example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc\""
