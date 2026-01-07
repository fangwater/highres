#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="stream_pairmm"
BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"

usage() {
  cat <<'EOF'
Usage:
  deploy_stream_pairmm.sh [--dir <path>]

Defaults:
  --dir $HOME/highres/stream_pairmm

Examples:
  bash scripts/deploy_stream_pairmm.sh
  bash scripts/deploy_stream_pairmm.sh --dir "$HOME/highres/stream_pairmm"
EOF
}

TARGET_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
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

if [[ -z "$TARGET_DIR" ]]; then
  TARGET_DIR="$HOME/highres/stream_pairmm"
fi

echo "[INFO] Building $BIN_NAME (release)"
cargo build --release --bin "$BIN_NAME"

echo "[INFO] Deploying $BIN_NAME to $TARGET_DIR"
mkdir -p "$TARGET_DIR"
cp "$BIN_PATH" "$TARGET_DIR/"
chmod +x "$TARGET_DIR/$BIN_NAME"

for conf in highres.toml log4rs.yaml; do
  if [[ -f "$ROOT_DIR/$conf" ]]; then
    rsync -a "$ROOT_DIR/$conf" "$TARGET_DIR/"
  fi
done

for cache in "$ROOT_DIR"/*_cache.json; do
  if [[ -f "$cache" ]]; then
    rsync -a "$cache" "$TARGET_DIR/"
  fi
done

mkdir -p "$TARGET_DIR/logs"

mkdir -p "$TARGET_DIR/scripts"
for script in start_stream_pairmm.sh stop_stream_pairmm.sh; do
  if [[ -f "$ROOT_DIR/scripts/$script" ]]; then
    rsync -a "$ROOT_DIR/scripts/$script" "$TARGET_DIR/scripts/"
    chmod +x "$TARGET_DIR/scripts/$script"
  fi
done

echo "[INFO] $BIN_NAME deployed to $TARGET_DIR"
echo "[INFO] Example: cd $TARGET_DIR && ./scripts/start_stream_pairmm.sh --ipc /tmp/mth_pubs/okex-futures-binance-futures/SOLUSDT.ipc"
