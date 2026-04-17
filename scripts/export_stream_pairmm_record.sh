#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

usage() {
  cat <<'EOF'
Usage:
  export_stream_pairmm_record.sh --symbol <SYMBOL> [--out-dir <DIR>] [--db-root <DIR>] [--profile <name>]
  export_stream_pairmm_record.sh --all [--out-dir <DIR>] [--db-root <DIR>] [--profile <name>]

Examples:
  ./scripts/export_stream_pairmm_record.sh --all
  ./scripts/export_stream_pairmm_record.sh --symbol SOLUSDT
  ./scripts/export_stream_pairmm_record.sh --profile okex-futures-binance-futures --symbol SOLUSDT
  ./scripts/export_stream_pairmm_record.sh --profile okex-futures-okex-futures --symbol SOLUSDT
  ./scripts/export_stream_pairmm_record.sh --profile binance-margin-binance-futures --symbol SOLUSDT
  ./scripts/export_stream_pairmm_record.sh --db-root /mnt/data/data/record_persist/pairmm/okex-futures-binance-futures --all
  ./scripts/export_stream_pairmm_record.sh --symbol SOLUSDT --out-dir ./exports
EOF
}

SUPPORTED_PROFILES=("okex-futures-binance-futures" "okex-futures-okex-futures" "binance-margin-binance-futures" "binance-futures-binance-futures")

infer_profile_from_base_dir() {
  local base_name=""
  local profile=""
  base_name="$(basename "$BASE_DIR")"

  for profile in "${SUPPORTED_PROFILES[@]}"; do
    if [[ "$base_name" == "$profile" ]] || [[ "$base_name" == *"-${profile}" ]]; then
      echo "$profile"
      return 0
    fi
  done

  for profile in "${SUPPORTED_PROFILES[@]}"; do
    if [[ -f "${BASE_DIR}/config.${profile}.toml" ]] || \
       [[ -f "${BASE_DIR}/highres.${profile}.toml" ]] || \
       [[ -f "${BASE_DIR}/pnlu_factor.${profile}.toml" ]]; then
      echo "$profile"
      return 0
    fi
  done

  return 1
}

derive_export_scope() {
  local profile="$1"
  local db_root="$2"
  local token=""

  if [[ -n "$profile" ]]; then
    token="$profile"
  else
    token="$(basename "${db_root%/}")"
  fi

  token="${token//\//-}"
  if [[ -z "$token" ]]; then
    token="default"
  fi
  echo "$token"
}

SYMBOL=""
ALL=false
OUT_DIR=""
DB_ROOT=""
PROFILE=""
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

ARGS=()
if [[ -z "$DB_ROOT" ]]; then
  if [[ -z "$PROFILE" ]]; then
    PROFILE="$(infer_profile_from_base_dir || true)"
    if [[ -n "$PROFILE" ]]; then
      echo "[INFO] inferred profile from working directory: ${PROFILE}"
    fi
  fi

  if [[ -n "$PROFILE" ]]; then
    DB_ROOT="/mnt/data/data/record_persist/pairmm/${PROFILE}"
  else
    echo "[ERROR] failed to infer profile from current working directory: ${BASE_DIR}" >&2
    echo "[HINT] provide --profile or --db-root explicitly" >&2
    exit 1
  fi
fi
ARGS+=(--db-root "$DB_ROOT")

if [[ -z "$OUT_DIR" ]]; then
  EXPORT_SCOPE="$(derive_export_scope "$PROFILE" "$DB_ROOT")"
  OUT_DIR="/mnt/data/order_data/${EXPORT_SCOPE}"
  echo "[INFO] auto out dir by scope=${EXPORT_SCOPE}: ${OUT_DIR}"
fi

mkdir -p "$OUT_DIR"

if [[ "$ALL" == "true" ]]; then
  echo "[INFO] Export all symbols"
  echo "[INFO] DB root: ${DB_ROOT}"
  echo "[INFO] Out dir: ${OUT_DIR}"
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
    echo "[INFO] Exporting ${SYMBOL}"
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
