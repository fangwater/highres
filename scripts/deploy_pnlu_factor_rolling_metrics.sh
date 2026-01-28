#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_NAME="pnlu_factor_rolling_metrics"

usage() {
  cat <<'EOF'
Usage:
  deploy_pnlu_factor_rolling_metrics.sh [--host <user@ip>] [--dir <path>]

Defaults:
  --host u171@10.61.10.32
  --dir /home/u171/mth_pub

Examples:
  bash scripts/deploy_pnlu_factor_rolling_metrics.sh
  bash scripts/deploy_pnlu_factor_rolling_metrics.sh --host u171@10.61.10.32
  bash scripts/deploy_pnlu_factor_rolling_metrics.sh --dir "/home/u171/mth_pub"
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

echo "[INFO] Building $BIN_NAME (release)"
cargo build --release --bin "$BIN_NAME"

echo "[INFO] Deploying binary to $TARGET_HOST:$TARGET_DIR"
ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR\""
BIN_PATH="$ROOT_DIR/target/release/$BIN_NAME"
rsync -a "$BIN_PATH" "$TARGET_HOST:$TARGET_DIR/"
ssh "$TARGET_HOST" "chmod +x \"$TARGET_DIR/$BIN_NAME\""

for conf in pnlu_factor_rolling.toml pnlu_factor_rolling_symbols.json log4rs.yaml; do
  if [[ -f "$ROOT_DIR/$conf" ]]; then
    rsync -a "$ROOT_DIR/$conf" "$TARGET_HOST:$TARGET_DIR/"
  fi
done

ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR/logs\""

for script in \
  start_pnlu_factor_rolling_metrics.sh \
  stop_pnlu_factor_rolling_metrics.sh; do
  if [[ -f "$ROOT_DIR/scripts/$script" ]]; then
    ssh "$TARGET_HOST" "mkdir -p \"$TARGET_DIR/scripts\""
    rsync -a "$ROOT_DIR/scripts/$script" "$TARGET_HOST:$TARGET_DIR/scripts/"
    ssh "$TARGET_HOST" "chmod +x \"$TARGET_DIR/scripts/$script\""
  fi
done

echo "[INFO] Binary deployed to $TARGET_HOST:$TARGET_DIR"
echo "[INFO] Example: ssh $TARGET_HOST \"cd $TARGET_DIR && ./scripts/start_pnlu_factor_rolling_metrics.sh\""
