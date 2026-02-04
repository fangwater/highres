#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
Print pnlu factor thresholds from Redis.

Reads per-symbol keys:
  <symbol><suffix>

Default redis_url and suffix are loaded from pnlu_factor_rolling.toml.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional
from datetime import datetime, timezone


def try_import_redis():
    try:
        import redis  # type: ignore
        return redis
    except Exception:
        return None


def try_import_toml():
    try:
        import tomllib  # type: ignore
        return tomllib
    except Exception:
        try:
            import tomli  # type: ignore
            return tomli
        except Exception:
            return None


def load_toml(path: str) -> Dict[str, Any]:
    mod = try_import_toml()
    if mod is None:
        raise RuntimeError("tomllib or tomli is required to read toml configs")
    if not Path(path).exists():
        return {}
    with open(path, "rb") as f:
        return mod.load(f)


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description="Print pnlu factor thresholds from Redis")
    p.add_argument("--config", default="pnlu_factor_rolling.toml")
    p.add_argument("--redis-url", default=os.environ.get("REDIS_URL"))
    p.add_argument("--host", default=os.environ.get("REDIS_HOST", "127.0.0.1"))
    p.add_argument("--port", type=int, default=int(os.environ.get("REDIS_PORT", 6379)))
    p.add_argument("--db", type=int, default=int(os.environ.get("REDIS_DB", 0)))
    p.add_argument("--password", default=os.environ.get("REDIS_PASSWORD"))
    p.add_argument("--suffix", help="redis key suffix, e.g. _pnlu_factor_thresholds")
    p.add_argument("--symbol", action="append", default=[])
    p.add_argument(
        "--scan-suffix",
        action="store_true",
        help="scan redis keys by suffix and print all matches",
    )
    p.add_argument(
        "--full",
        action="store_true",
        help="print full JSON payload per symbol (one block per key)",
    )
    p.add_argument(
        "--raw",
        action="store_true",
        help="print raw redis value (bytes/str) before decoding JSON",
    )
    return p.parse_args()


def load_default_redis_conf(config_path: str) -> Dict[str, str]:
    data = load_toml(config_path)
    return {
        "redis_url": str(data.get("redis_url", "")).strip(),
        "redis_key": str(data.get("redis_key", "")).strip(),
    }


def load_online_symbols() -> List[str]:
    data = load_toml("config.toml")
    symbols = data.get("online_symbols") or []
    return [str(s) for s in symbols]


def format_float(value: Any) -> str:
    if value is None:
        return ""
    try:
        return f"{float(value):.9f}"
    except Exception:
        return str(value)


def format_list(values: Any) -> str:
    if not isinstance(values, list):
        return ""
    return ",".join(format_float(v) for v in values)


def format_ts(value: Any) -> str:
    if value is None:
        return ""
    try:
        ts = int(value)
    except Exception:
        return ""
    if ts <= 0:
        return ""
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S")


def decode_raw_value(raw: Any) -> tuple[str, bytes]:
    if isinstance(raw, bytes):
        return raw.decode("utf-8", "ignore"), raw
    if isinstance(raw, str):
        return raw, raw.encode("utf-8", "ignore")
    text = str(raw)
    return text, text.encode("utf-8", "ignore")


def print_raw_value(symbol: str, raw_bytes: bytes) -> None:
    hex_str = raw_bytes.hex()
    print(f"[{symbol}] raw_bytes_len={len(raw_bytes)}")
    print(hex_str)


def print_three_line_table(headers: List[str], rows: List[List[str]]) -> None:
    widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            widths[i] = max(widths[i], len(cell))

    def fmt_row(values: List[str]) -> str:
        return "  ".join(values[i].ljust(widths[i]) for i in range(len(values)))

    header_line = fmt_row(headers)
    top_rule = "=" * len(header_line)
    mid_rule = "-" * len(header_line)
    bot_rule = "=" * len(header_line)

    print(top_rule)
    print(header_line)
    print(mid_rule)
    for row in rows:
        print(fmt_row(row))
    print(bot_rule)


def main() -> int:
    args = parse_args()
    redis = try_import_redis()
    if redis is None:
        print("redis package not installed, run: pip install redis", file=sys.stderr)
        return 2

    defaults = load_default_redis_conf(args.config)
    redis_url = args.redis_url or defaults.get("redis_url")
    suffix = args.suffix or defaults.get("redis_key", "")

    if not redis_url:
        print("redis_url is required (config or --redis-url)", file=sys.stderr)
        return 2
    if not suffix:
        print("redis key suffix is required (config or --suffix)", file=sys.stderr)
        return 2

    rds = redis.from_url(redis_url) if redis_url else redis.Redis(
        host=args.host, port=args.port, db=args.db, password=args.password
    )

    print(f"Redis: {redis_url}")
    try:
        rds.ping()
        print("Redis: connected")
    except Exception as exc:
        print(f"Redis: connect failed: {exc}", file=sys.stderr)
        return 2
    print(f"Key suffix: {suffix}\n")

    headers = ["symbol", "ts_utc", "target_ts_utc", "factor", "quantiles", "thresholds", "ready"]
    rows: List[List[str]] = []
    missing: List[str] = []

    if args.scan_suffix:
        pattern = f"*{suffix}"
        keys = list(rds.scan_iter(match=pattern))
        symbols = []
        for key in keys:
            if isinstance(key, bytes):
                key = key.decode("utf-8", "ignore")
            if not key.endswith(suffix):
                continue
            symbols.append(key[: -len(suffix)])
        symbols.sort()
    else:
        symbols = args.symbol or load_online_symbols()

    if not symbols:
        print("no symbols found for output", file=sys.stderr)
        return 2

    for symbol in symbols:
        key = f"{symbol}{suffix}"
        raw = rds.get(key)
        if raw is None:
            missing.append(symbol)
            continue
        raw_text, raw_bytes = decode_raw_value(raw)
        if args.raw:
            print_raw_value(symbol, raw_bytes)
        try:
            payload = json.loads(raw_text)
        except Exception:
            if args.full:
                print(f"[{symbol}] invalid_json")
            else:
                rows.append([symbol, "", "", "", "invalid_json"])
            continue
        if args.full:
            print(f"[{symbol}]")
            print(json.dumps(payload, indent=2, sort_keys=True))
        else:
            rows.append(
                [
                    symbol,
                    format_ts(payload.get("ts")),
                    format_ts(payload.get("target_ts")),
                    format_float(payload.get("factor")),
                    format_list(payload.get("quantiles")),
                    format_list(payload.get("thresholds")),
                    str(payload.get("ready", "")),
                ]
            )

    if rows and not args.full:
        print_three_line_table(headers, rows)
    if missing:
        print(f"\nMissing: {', '.join(missing)}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
