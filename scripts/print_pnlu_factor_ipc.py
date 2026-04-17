#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
Print pnlu_factor IPC messages from ZeroMQ.

Default topic:
  pnlu_factor

Default endpoint by profile:
  ipc:///tmp/mth_pubs/pnlu_factor/<profile>.ipc
"""

from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, Optional

SUPPORTED_PROFILES = (
    "okex-futures-binance-futures",
    "okex-futures-okex-futures",
    "binance-margin-binance-futures",
    "binance-futures-binance-futures",
)


def try_import_zmq():
    try:
        import zmq  # type: ignore
        return zmq
    except Exception:
        return None


def sanitize_profile_token(raw: str) -> str:
    token = raw.strip().strip("/")
    token = token.split("/")[-1]
    if token.endswith(".toml"):
        token = token[:-5]
    token = token.lower().replace("_", "-").replace("/", "-").strip("-")
    if "-" in token:
        left, right = token.split("-", 1)
        if left.isdigit() and right:
            token = right
    return token or "default"


def endpoint_from_profile(profile: str) -> str:
    token = sanitize_profile_token(profile)
    return f"ipc:///tmp/mth_pubs/pnlu_factor/{token}.ipc"


def infer_profile_from_cwd() -> Optional[str]:
    cwd = Path.cwd()
    candidates = [cwd.name, str(cwd)]
    for raw in candidates:
        token = sanitize_profile_token(raw)
        for profile in SUPPORTED_PROFILES:
            if token == profile or token.endswith(f"-{profile}") or raw.endswith(profile):
                return profile
    return None


def format_ts(ts: Any) -> str:
    if ts is None:
        return ""
    try:
        value = int(ts)
    except Exception:
        return str(ts)
    return datetime.fromtimestamp(value, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


def format_float(value: Any) -> str:
    if value is None:
        return ""
    try:
        return f"{float(value):.9f}"
    except Exception:
        return str(value)


def print_row(payload: Dict[str, Any]) -> None:
    symbol = str(payload.get("symbol", ""))
    ts = format_ts(payload.get("ts"))
    target_ts = format_ts(payload.get("target_ts"))
    pnlu_sum = format_float(payload.get("pnlu_sum"))
    factor = format_float(payload.get("factor"))
    print(
        f"symbol={symbol} ts_utc={ts} target_ts_utc={target_ts} "
        f"pnlu_sum={pnlu_sum} factor={factor}"
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Print pnlu_factor IPC messages")
    parser.add_argument(
        "--profile",
        help="profile name used to derive IPC endpoint; if omitted, infer from current directory",
    )
    parser.add_argument("--ipc", help="explicit IPC endpoint, e.g. ipc:///tmp/mth_pubs/pnlu_factor/okex-futures-binance-futures.ipc")
    parser.add_argument("--topic", default="pnlu_factor", help="subscription topic")
    parser.add_argument("--count", type=int, help="stop after receiving N messages")
    parser.add_argument("--raw", action="store_true", help="print raw JSON payload")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    zmq = try_import_zmq()
    if zmq is None:
        print("pyzmq is required, install with: pip install pyzmq", file=sys.stderr)
        return 2

    profile = args.profile or infer_profile_from_cwd()
    endpoint = args.ipc or (endpoint_from_profile(profile) if profile else "")
    if not endpoint:
        print("IPC endpoint is required (--ipc or --profile, or run inside a profile dir)", file=sys.stderr)
        return 2

    ctx = zmq.Context()
    socket = ctx.socket(zmq.SUB)
    socket.setsockopt(zmq.SUBSCRIBE, args.topic.encode("utf-8"))
    socket.connect(endpoint)

    print(f"IPC: {endpoint}")
    print(f"Topic: {args.topic}")
    if profile:
        print(f"Profile: {profile}")
    print("Waiting for messages...", file=sys.stderr)

    received = 0
    try:
        while True:
            parts = socket.recv_multipart()
            if len(parts) < 2:
                print(f"skip invalid multipart parts={len(parts)}", file=sys.stderr)
                continue
            topic = parts[0].decode("utf-8", "ignore")
            body = parts[1].decode("utf-8", "ignore")
            try:
                payload = json.loads(body)
            except Exception:
                print(f"[{topic}] {body}")
                continue

            if args.raw:
                print(json.dumps(payload, ensure_ascii=True, sort_keys=True))
            else:
                print_row(payload)

            received += 1
            if args.count is not None and received >= args.count:
                break
    except KeyboardInterrupt:
        return 130
    finally:
        socket.close(0)
        ctx.term()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
