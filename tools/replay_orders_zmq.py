#!/usr/bin/env python3
import argparse
import csv
import json
import time
from pathlib import Path

import zmq


def parse_args():
    parser = argparse.ArgumentParser(
        description="Replay orders CSV to a ZMQ PUB socket."
    )
    parser.add_argument("--input", required=True, help="Orders CSV path.")
    parser.add_argument(
        "--ipc-prefix",
        default="ipc:///tmp/mth_pubs/stream_pairmm/okex-futures-binance-futures",
        help="IPC prefix, default matches record/stream processes.",
    )
    parser.add_argument(
        "--symbol",
        default=None,
        help="Override symbol; otherwise read from CSV column 5.",
    )
    parser.add_argument(
        "--mode",
        choices=["all", "warmup", "live"],
        default="all",
        help="Send all rows or split by tail minutes.",
    )
    parser.add_argument(
        "--tail-minutes",
        type=int,
        default=10,
        help="Tail minutes for warmup/live split.",
    )
    parser.add_argument(
        "--sleep-ms",
        type=int,
        default=0,
        help="Sleep between sends (ms).",
    )
    return parser.parse_args()


def normalize_ipc_prefix(raw: str) -> str:
    prefixed = raw if raw.startswith("ipc://") else f"ipc://{raw}"
    return prefixed.rstrip("/")


def get_ts_ms(row):
    try:
        update_ts = int(row[2])
    except Exception:
        update_ts = 0
    if update_ts > 0:
        return update_ts
    try:
        return int(row[1])
    except Exception:
        return 0


def get_symbol(row):
    try:
        return row[4]
    except Exception:
        return None


def parse_row(row):
    return {
        "client_order_id": row[0],
        "create_ts": int(row[1]),
        "update_ts": int(row[2]),
        "symbol": row[4],
        "ttype": row[5],
        "sid": int(row[6]),
        "side": row[7],
        "price": float(row[8]),
        "amount_init": float(row[9]),
        "amount_update": float(row[10]),
        "status": row[11],
        "inpos": float(row[12]),
        "tlen": float(row[13]),
        "from_key": row[14],
        "bid1": float(row[15]),
        "ask1": float(row[16]),
    }


def main():
    args = parse_args()
    path = Path(args.input)
    if not path.exists():
        raise SystemExit(f"input not found: {path}")

    max_ts = None
    if args.mode in ("warmup", "live"):
        with path.open(newline="") as f:
            reader = csv.reader(f)
            for row in reader:
                if len(row) < 17:
                    continue
                ts = get_ts_ms(row)
                if max_ts is None or ts > max_ts:
                    max_ts = ts
        if max_ts is None:
            raise SystemExit("failed to find max timestamp")

    split_ts = None
    if max_ts is not None:
        split_ts = max_ts - args.tail_minutes * 60 * 1000

    symbol = args.symbol
    if symbol is None:
        with path.open(newline="") as f:
            reader = csv.reader(f)
            for row in reader:
                if len(row) < 17:
                    continue
                symbol = get_symbol(row)
                break
    if not symbol:
        raise SystemExit("symbol not found")

    endpoint = f"{normalize_ipc_prefix(args.ipc_prefix)}/{symbol}.ipc"
    ctx = zmq.Context()
    sock = ctx.socket(zmq.PUB)
    sock.bind(endpoint)
    time.sleep(0.5)

    sent = 0
    with path.open(newline="") as f:
        reader = csv.reader(f)
        for row in reader:
            if len(row) < 17:
                continue
            ts = get_ts_ms(row)
            if split_ts is not None:
                if args.mode == "warmup" and ts > split_ts:
                    continue
                if args.mode == "live" and ts <= split_ts:
                    continue
            payload = parse_row(row)
            msg = json.dumps(payload).encode("utf-8")
            sock.send_multipart([b"orders", msg])
            sent += 1
            if args.sleep_ms > 0:
                time.sleep(args.sleep_ms / 1000.0)
    print(f"sent={sent} endpoint={endpoint} mode={args.mode}")


if __name__ == "__main__":
    main()
