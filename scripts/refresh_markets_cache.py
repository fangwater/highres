#!/usr/bin/env python3
import argparse
import json
import os
import sys
import urllib.request


CACHE_TYPE_MAP = {
    "spot": "Spot",
    "linear_future": "LinearFuture",
    "inverse_future": "InverseFuture",
    "linear_swap": "LinearSwap",
    "inverse_swap": "InverseSwap",
    "american_option": "AmericanOption",
    "european_option": "EuropeanOption",
    "quanto_future": "QuantoFuture",
    "quanto_swap": "QuantoSwap",
    "move": "Move",
    "bvol": "BVOL",
}


def parse_list(value):
    return [item.strip() for item in value.split(",") if item.strip()]


def build_opener(http_proxy, https_proxy):
    proxies = None
    if http_proxy or https_proxy:
        proxies = {}
        if http_proxy:
            proxies["http"] = http_proxy
            if not https_proxy:
                proxies["https"] = http_proxy
        if https_proxy:
            proxies["https"] = https_proxy
    return urllib.request.build_opener(urllib.request.ProxyHandler(proxies))


def fetch_json(opener, url):
    req = urllib.request.Request(url, headers={"User-Agent": "markets-cache-refresh/1.0"})
    with opener.open(req, timeout=30) as resp:
        data = resp.read().decode("utf-8")
    return json.loads(data)


def to_float(value):
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def prune_none(obj):
    return {k: v for k, v in obj.items() if v is not None}


def fetch_binance_linear_swap(opener):
    url = "https://fapi.binance.com/fapi/v1/exchangeInfo"
    data = fetch_json(opener, url)
    symbols = data.get("symbols", [])
    markets = []
    for sym in symbols:
        if sym.get("contractType") != "PERPETUAL":
            continue
        symbol = sym.get("symbol")
        base = (sym.get("baseAsset") or "").upper()
        quote = (sym.get("quoteAsset") or "").upper()
        settle = (sym.get("marginAsset") or quote).upper()
        if not symbol or not base or not quote:
            continue
        tick_size = None
        lot_size = None
        min_qty = None
        max_qty = None
        for f in sym.get("filters", []):
            ftype = f.get("filterType")
            if ftype == "PRICE_FILTER":
                tick_size = to_float(f.get("tickSize"))
            elif ftype == "LOT_SIZE":
                lot_size = to_float(f.get("stepSize"))
                min_qty = to_float(f.get("minQty"))
                max_qty = to_float(f.get("maxQty"))
        if not tick_size or not lot_size:
            continue
        market = {
            "exchange": "binance",
            "market_type": "linear_swap",
            "symbol": symbol,
            "base_id": base,
            "quote_id": quote,
            "settle_id": settle,
            "base": base,
            "quote": quote,
            "settle": settle,
            "active": sym.get("status") == "TRADING",
            "margin": True,
            "fees": {"maker": 0.0002, "taker": 0.0004},
            "precision": {"tick_size": tick_size, "lot_size": lot_size},
            "quantity_limit": prune_none({"min": min_qty, "max": max_qty}) or None,
            "contract_value": 1.0,
            "info": sym,
        }
        markets.append(prune_none(market))
    return markets


def fetch_okx_linear_swap(opener):
    url = "https://www.okx.com/api/v5/public/instruments?instType=SWAP"
    data = fetch_json(opener, url)
    if str(data.get("code")) not in ("0", "None"):
        raise RuntimeError("okx api error: %s" % data.get("msg"))
    markets = []
    for inst in data.get("data", []):
        if inst.get("ctType") and inst.get("ctType") != "linear":
            continue
        symbol = inst.get("instId") or ""
        parts = symbol.split("-")
        base = parts[0].upper() if len(parts) >= 2 else (inst.get("ctValCcy") or "").upper()
        quote = parts[1].upper() if len(parts) >= 2 else (inst.get("settleCcy") or "").upper()
        settle = (inst.get("settleCcy") or quote).upper()
        if not symbol or not base or not quote:
            continue
        tick_size = to_float(inst.get("tickSz"))
        lot_size = to_float(inst.get("lotSz"))
        min_sz = to_float(inst.get("minSz"))
        ct_val = to_float(inst.get("ctVal"))
        if not tick_size or not lot_size or ct_val is None:
            continue
        market = {
            "exchange": "okx",
            "market_type": "linear_swap",
            "symbol": symbol,
            "base_id": base,
            "quote_id": quote,
            "settle_id": settle,
            "base": base,
            "quote": quote,
            "settle": settle,
            "active": inst.get("state") == "live",
            "margin": True,
            "fees": {"maker": 0.0002, "taker": 0.0005},
            "precision": {"tick_size": tick_size, "lot_size": lot_size},
            "quantity_limit": prune_none({"min": min_sz}) or None,
            "contract_value": ct_val,
            "info": inst,
        }
        markets.append(prune_none(market))
    return markets


def cache_filename(exchange, market_type):
    if market_type not in CACHE_TYPE_MAP:
        raise ValueError("unsupported market type: %s" % market_type)
    return "%s_%s_cache.json" % (exchange, CACHE_TYPE_MAP[market_type])


def write_cache(path, markets):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(markets, f, separators=(",", ":"), ensure_ascii=True)


def refresh(exchange, market_type, opener, workdir):
    if exchange == "binance" and market_type == "linear_swap":
        markets = fetch_binance_linear_swap(opener)
    elif exchange == "okx" and market_type == "linear_swap":
        markets = fetch_okx_linear_swap(opener)
    else:
        raise ValueError("unsupported exchange/type: %s/%s" % (exchange, market_type))
    if not markets:
        raise RuntimeError("no markets fetched for %s %s" % (exchange, market_type))
    out_path = os.path.join(workdir, cache_filename(exchange, market_type))
    write_cache(out_path, markets)
    print("[OK] wrote %s (%d markets)" % (out_path, len(markets)))


def main():
    parser = argparse.ArgumentParser(description="Refresh market cache files.")
    parser.add_argument(
        "--exchanges",
        default="okx,binance",
        help="comma-separated exchanges, default: okx,binance",
    )
    parser.add_argument(
        "--market-types",
        default="linear_swap",
        help="comma-separated market types, default: linear_swap",
    )
    parser.add_argument("--workdir", default=".", help="output directory for cache files")
    parser.add_argument("--http-proxy", default=None, help="http proxy, e.g. http://127.0.0.1:7890")
    parser.add_argument("--https-proxy", default=None, help="https proxy, e.g. http://127.0.0.1:7890")
    args = parser.parse_args()

    workdir = os.path.abspath(args.workdir)
    if not os.path.isdir(workdir):
        print("[ERROR] workdir not found: %s" % workdir, file=sys.stderr)
        return 2

    opener = build_opener(args.http_proxy, args.https_proxy)
    exchanges = parse_list(args.exchanges)
    market_types = parse_list(args.market_types)
    if not exchanges or not market_types:
        print("[ERROR] exchanges/market-types cannot be empty", file=sys.stderr)
        return 2

    for exchange in exchanges:
        for market_type in market_types:
            refresh(exchange, market_type, opener, workdir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
