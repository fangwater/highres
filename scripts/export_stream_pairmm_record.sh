#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  export_stream_pairmm_record.sh --symbol <SYMBOL> [--out-dir <DIR>] [--db-root <DIR>]
  export_stream_pairmm_record.sh --all [--out-dir <DIR>] [--db-root <DIR>]

Examples:
  ./scripts/export_stream_pairmm_record.sh --symbol SOLUSDT
  ./scripts/export_stream_pairmm_record.sh --symbol SOLUSDT --out-dir ./exports
  ./scripts/export_stream_pairmm_record.sh --all
EOF
}

SYMBOL=""
ALL=false
OUT_DIR=""
DB_ROOT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --symbol)
      SYMBOL="${2:-}"
      if [[ -z "$SYMBOL" ]]; then
        echo "[ERROR] --symbol requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --all)
      ALL=true
      shift
      ;;
    --out-dir)
      OUT_DIR="${2:-}"
      if [[ -z "$OUT_DIR" ]]; then
        echo "[ERROR] --out-dir requires a value" >&2
        usage >&2
        exit 1
      fi
      shift 2
      ;;
    --db-root)
      DB_ROOT="${2:-}"
      if [[ -z "$DB_ROOT" ]]; then
        echo "[ERROR] --db-root requires a value" >&2
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

if [[ "$ALL" == "true" && -n "$SYMBOL" ]]; then
  echo "[ERROR] --all cannot be used with --symbol" >&2
  usage >&2
  exit 1
fi

if [[ "$ALL" != "true" && -z "$SYMBOL" ]]; then
  echo "[ERROR] --symbol is required" >&2
  usage >&2
  exit 1
fi

if [[ -z "$OUT_DIR" ]]; then
  OUT_DIR="${BASE_DIR}/order_data"
fi

BIN_CANDIDATES=(
  "${BASE_DIR}/stream_pairmm_record"
  "${BASE_DIR}/target/release/stream_pairmm_record"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] stream_pairmm_record binary not found. Build first with: cargo build --release --bin stream_pairmm_record" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

ARGS=()
if [[ "$ALL" == "true" && -z "$DB_ROOT" ]]; then
  DB_ROOT="${BASE_DIR}/data/record_persist/pairmm/okex-futures-binance-futures"
fi
if [[ -n "$DB_ROOT" ]]; then
  ARGS+=(--db-root "$DB_ROOT")
fi

if [[ "$ALL" == "true" ]]; then
  if [[ ! -d "$DB_ROOT" ]]; then
    echo "[ERROR] db root not found: $DB_ROOT" >&2
    exit 1
  fi
  shopt -s nullglob
  for sym_dir in "$DB_ROOT"/*; do
    if [[ ! -d "$sym_dir" ]]; then
      continue
    fi
    SYMBOL="$(basename "$sym_dir")"
    ORDERS_OUT="${OUT_DIR}/${SYMBOL}_orders.csv"
    NPS_OUT="${OUT_DIR}/${SYMBOL}_nps.csv"
    "$BIN_PATH" --export --symbol "$SYMBOL" --kind orders --out "$ORDERS_OUT" "${ARGS[@]}"
    "$BIN_PATH" --export --symbol "$SYMBOL" --kind nps --out "$NPS_OUT" "${ARGS[@]}"
    echo "[INFO] Exported ${SYMBOL}:"
    echo "  ${ORDERS_OUT}"
    echo "  ${NPS_OUT}"
  done
  exit 0
fi

ORDERS_OUT="${OUT_DIR}/${SYMBOL}_orders.csv"
NPS_OUT="${OUT_DIR}/${SYMBOL}_nps.csv"

"$BIN_PATH" --export --symbol "$SYMBOL" --kind orders --out "$ORDERS_OUT" "${ARGS[@]}"
"$BIN_PATH" --export --symbol "$SYMBOL" --kind nps --out "$NPS_OUT" "${ARGS[@]}"

echo "[INFO] Exported:"
echo "  ${ORDERS_OUT}"
echo "  ${NPS_OUT}"
