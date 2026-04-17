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

RECORD_RETENTION_SECS=259200
RECORD_CLEANUP_INTERVAL_SECS=7200

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

usage() {
  cat <<'EOF'
Usage:
  start_stream_pairmm_record.sh --profile <name> [--name <pm2_name>] [--ipc-prefix <ipc>] [--db-root <path>]
  start_stream_pairmm_record.sh --all

Examples:
  ./scripts/start_stream_pairmm_record.sh --profile okex-futures-binance-futures
  ./scripts/start_stream_pairmm_record.sh --profile okex-futures-okex-futures
  ./scripts/start_stream_pairmm_record.sh --profile binance-margin-binance-futures
  ./scripts/start_stream_pairmm_record.sh --profile binance-futures-binance-futures
  ./scripts/start_stream_pairmm_record.sh --all
EOF
}

if ! command -v pm2 >/dev/null 2>&1; then
  echo "[ERROR] pm2 not found, please install pm2 first" >&2
  exit 1
fi

NAME_OVERRIDE=""
PROFILE=""
IPC_PREFIX=""
DB_ROOT=""
IPC_PREFIX_SET="0"
DB_ROOT_SET="0"
START_ALL="0"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --all)
      START_ALL="1"
      shift
      ;;
    --name)
      NAME_OVERRIDE="${2:-}"
      if [[ -z "$NAME_OVERRIDE" ]]; then
        echo "[ERROR] --name requires a value" >&2
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
    --db-root)
      DB_ROOT="${2:-}"
      if [[ -z "$DB_ROOT" ]]; then
        echo "[ERROR] --db-root requires a value" >&2
        usage >&2
        exit 1
      fi
      DB_ROOT_SET="1"
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

normalize_ipc_prefix() {
  local ipc="$1"
  ipc="${ipc#ipc://}"
  ipc="${ipc%/}"
  echo "$ipc"
}

profile_from_ipc_prefix() {
  local ipc="$1"
  local normalized=""
  normalized="$(normalize_ipc_prefix "$ipc")"
  local record_root="/tmp/mth_pubs/stream_pairmm/"
  if [[ -z "$normalized" ]]; then
    return 1
  fi
  if [[ "${normalized}" != ${record_root}* ]]; then
    return 1
  fi
  local profile="${normalized#${record_root}}"
  if [[ -z "$profile" ]] || [[ "$profile" == *"/"* ]]; then
    return 1
  fi
  echo "$profile"
}

if [[ "$START_ALL" == "1" ]]; then
  if [[ -n "$PROFILE" ]] || [[ -n "$NAME_OVERRIDE" ]] || [[ "$IPC_PREFIX_SET" == "1" ]] || [[ "$DB_ROOT_SET" == "1" ]]; then
    echo "[ERROR] --all cannot be used with --profile/--name/--ipc-prefix/--db-root" >&2
    exit 1
  fi
  for p in "${SUPPORTED_PROFILES[@]}"; do
    echo "[INFO] start recorder profile=${p}"
    bash "$0" --profile "$p"
  done
  exit 0
fi

if [[ -z "$PROFILE" ]]; then
  echo "[ERROR] --profile is required (or use --all)" >&2
  usage >&2
  exit 1
fi

if ! is_supported_profile "$PROFILE"; then
  echo "[ERROR] unsupported --profile: ${PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi

if [[ "$IPC_PREFIX_SET" == "0" ]]; then
  IPC_PREFIX="/tmp/mth_pubs/stream_pairmm/${PROFILE}"
fi

if [[ -n "$NAME_OVERRIDE" ]]; then
  NAME="$NAME_OVERRIDE"
else
  NAME="stream_pairmm_record-$(profile_alias "$PROFILE")"
fi

NAMESPACE="$(basename "${BASE_DIR}")"

IPC_PROFILE="$(profile_from_ipc_prefix "$IPC_PREFIX" || true)"
if [[ -z "$IPC_PROFILE" ]]; then
  echo "[ERROR] invalid --ipc-prefix: ${IPC_PREFIX}" >&2
  exit 1
fi
if ! is_supported_profile "$IPC_PROFILE"; then
  echo "[ERROR] --ipc-prefix profile is unsupported: ${IPC_PROFILE}" >&2
  echo "[ERROR] supported: ${SUPPORTED_PROFILES[*]}" >&2
  exit 1
fi
if [[ "$IPC_PROFILE" != "$PROFILE" ]]; then
  echo "[ERROR] --profile (${PROFILE}) does not match --ipc-prefix profile (${IPC_PROFILE})" >&2
  exit 1
fi

if [[ "$DB_ROOT_SET" == "0" ]]; then
  DB_ROOT="/mnt/data/data/record_persist/pairmm/${IPC_PROFILE}"
fi

BIN_CANDIDATES=(
  "${BASE_DIR}/stream_pairmm_record"
  "${BASE_DIR}/target/release/stream_pairmm_record"
)

BIN_PATH=""
for cand in "${BIN_CANDIDATES[@]}"; do
  if [[ -f "$cand" && -x "$cand" ]]; then
    BIN_PATH="$cand"
    break
  fi
done

if [[ -z "$BIN_PATH" ]]; then
  echo "[ERROR] stream_pairmm_record binary not found. Build first with: cargo build --release --bin stream_pairmm_record" >&2
  exit 1
fi

ARGS=()
ARGS+=(--ipc-prefix "$IPC_PREFIX")
ARGS+=(--db-root "$DB_ROOT")
ARGS+=(--retention-secs "$RECORD_RETENTION_SECS")
ARGS+=(--cleanup-interval-secs "$RECORD_CLEANUP_INTERVAL_SECS")

echo "[INFO] Restarting ${NAME}"
pm2 delete "stream_pairmm_record-${PROFILE}" --namespace "$NAMESPACE" >/dev/null 2>&1 || true
pm2 delete "$NAME" --namespace "$NAMESPACE" >/dev/null 2>&1 || true

PM2_CMD=(pm2 start "$BIN_PATH" --name "$NAME" --namespace "$NAMESPACE" --cwd "$BASE_DIR")
if [[ ${#ARGS[@]} -gt 0 ]]; then
  PM2_CMD+=(-- "${ARGS[@]}")
fi

RUST_LOG="${RUST_LOG:-info}" "${PM2_CMD[@]}"

echo ""
echo "[INFO] Started: ${NAME}"
echo "Profile: ${PROFILE}"
echo "IPC Prefix: ${IPC_PREFIX}"
echo "DB Root: ${DB_ROOT}"
echo "Retention Secs: ${RECORD_RETENTION_SECS}"
echo "Cleanup Interval Secs: ${RECORD_CLEANUP_INTERVAL_SECS}"
echo "Namespace: ${NAMESPACE}"
echo "Logs: pm2 logs --namespace ${NAMESPACE} ${NAME}"
echo "Status: pm2 status --namespace ${NAMESPACE}"
