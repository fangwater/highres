#!/usr/bin/env python3
# -*- coding: utf-8 -*-

"""
Monitor pnlu_factor IPC messages and expose a WebSocket dashboard.

Status rules:
  - online: latest payload has factor and message is fresh
  - warming: no factor yet, or latest payload has no factor but stream is fresh
  - offline: factor had appeared before, then no message is received for N seconds
"""

from __future__ import annotations

import argparse
import asyncio
import json
import signal
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Set

import tomli
import zmq
import zmq.asyncio
from aiohttp import WSMsgType, web

SUPPORTED_PROFILES = (
    "okex-futures-binance-futures",
    "binance-margin-binance-futures",
    "binance-futures-binance-futures",
)


HTML_PAGE = """<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>Pnlu Factor Monitor</title>
  <style>
    :root {
      --bg-0: #06121a;
      --bg-1: #0e2330;
      --bg-2: rgba(8, 24, 34, 0.72);
      --panel: rgba(10, 22, 31, 0.82);
      --line: rgba(162, 196, 214, 0.15);
      --text: #eef5f7;
      --muted: #9eb2bc;
      --cold: #75b8d1;
      --ok: #84d494;
      --warn: #f4be64;
      --down: #f07d6d;
      --shadow: 0 30px 60px rgba(0, 0, 0, 0.30);
      --radius-lg: 28px;
      --radius-md: 20px;
      --radius-sm: 14px;
      --card-top: rgba(255, 255, 255, 0.05);
      --card-bottom: rgba(255, 255, 255, 0.01);
      --font-ui: "Avenir Next", "Segoe UI Variable", "Helvetica Neue", sans-serif;
      --font-display: "Iowan Old Style", "Palatino Linotype", "Book Antiqua", serif;
    }

    * {
      box-sizing: border-box;
    }

    html, body {
      margin: 0;
      min-height: 100%;
      background:
        radial-gradient(circle at top left, rgba(88, 170, 165, 0.22), transparent 28%),
        radial-gradient(circle at top right, rgba(237, 164, 102, 0.18), transparent 30%),
        linear-gradient(180deg, var(--bg-1), var(--bg-0) 62%);
      color: var(--text);
      font-family: var(--font-ui);
    }

    body {
      padding: 24px;
    }

    .shell {
      max-width: 1320px;
      margin: 0 auto;
      display: grid;
      gap: 18px;
    }

    .hero {
      position: relative;
      overflow: hidden;
      padding: 24px;
      border: 1px solid var(--line);
      border-radius: var(--radius-lg);
      background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0.02)),
        linear-gradient(120deg, rgba(17, 49, 64, 0.88), rgba(8, 19, 29, 0.94));
      box-shadow: var(--shadow);
    }

    .hero::after {
      content: "";
      position: absolute;
      inset: auto -8% -42% auto;
      width: 340px;
      height: 340px;
      border-radius: 50%;
      background: radial-gradient(circle, rgba(138, 210, 157, 0.18), transparent 66%);
      pointer-events: none;
    }

    .eyebrow {
      display: inline-flex;
      align-items: center;
      gap: 10px;
      padding: 7px 12px;
      border-radius: 999px;
      background: rgba(255, 255, 255, 0.06);
      color: var(--cold);
      font-size: 12px;
      letter-spacing: 0.14em;
      text-transform: uppercase;
    }

    .hero h1 {
      margin: 16px 0 10px;
      font-family: var(--font-display);
      font-size: clamp(34px, 6vw, 64px);
      line-height: 0.95;
      font-weight: 600;
      letter-spacing: -0.04em;
    }

    .hero p {
      margin: 0;
      max-width: 760px;
      color: var(--muted);
      font-size: 15px;
      line-height: 1.6;
    }

    .meta-grid, .summary-grid, .cards {
      display: grid;
      gap: 14px;
    }

    .meta-grid {
      grid-template-columns: repeat(4, minmax(0, 1fr));
      margin-top: 20px;
    }

    .summary-grid {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }

    .cards {
      grid-template-columns: repeat(4, minmax(0, 1fr));
    }

    .panel, .stat, .card {
      border: 1px solid var(--line);
      border-radius: var(--radius-md);
      background:
        linear-gradient(180deg, var(--card-top), var(--card-bottom)),
        var(--panel);
      backdrop-filter: blur(12px);
      box-shadow: var(--shadow);
    }

    .panel, .stat {
      padding: 18px;
    }

    .panel .label, .stat .label {
      color: var(--muted);
      font-size: 12px;
      letter-spacing: 0.08em;
      text-transform: uppercase;
    }

    .panel .value, .stat .value {
      margin-top: 10px;
      font-size: 22px;
      font-weight: 600;
    }

    .stat.ok .value, .pill.online { color: var(--ok); }
    .stat.warming .value, .pill.warming { color: var(--warn); }
    .stat.offline .value, .pill.offline { color: var(--down); }

    .toolbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 4px 2px;
    }

    .toolbar h2 {
      margin: 0;
      font-size: 16px;
      letter-spacing: 0.04em;
      text-transform: uppercase;
      color: var(--muted);
    }

    .toolbar .legend {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }

    .pill {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 8px 12px;
      border-radius: 999px;
      border: 1px solid var(--line);
      background: rgba(255, 255, 255, 0.04);
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }

    .dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: currentColor;
      box-shadow: 0 0 18px currentColor;
    }

    .card {
      position: relative;
      overflow: hidden;
      padding: 18px;
      min-height: 172px;
      transform: translateY(8px);
      opacity: 0;
      animation: rise 480ms ease forwards;
    }

    .card::after {
      content: "";
      position: absolute;
      inset: auto -10% -18% auto;
      width: 110px;
      height: 110px;
      border-radius: 50%;
      background: radial-gradient(circle, rgba(255, 255, 255, 0.08), transparent 68%);
      pointer-events: none;
    }

    .card.online { border-color: rgba(132, 212, 148, 0.22); }
    .card.warming { border-color: rgba(244, 190, 100, 0.24); }
    .card.offline { border-color: rgba(240, 125, 109, 0.24); }
    .card-head {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 12px;
      margin-bottom: 22px;
    }

    .symbol {
      font-size: 24px;
      font-weight: 700;
      letter-spacing: -0.03em;
    }

    .caption {
      margin-top: 6px;
      color: var(--muted);
      font-size: 13px;
    }

    .fact {
      display: grid;
      gap: 10px;
    }

    .row {
      display: flex;
      align-items: baseline;
      justify-content: space-between;
      gap: 12px;
      padding-bottom: 8px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    }

    .row:last-child {
      border-bottom: 0;
      padding-bottom: 0;
    }

    .row .k {
      color: var(--muted);
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
    }

    .row .v {
      font-size: 14px;
      text-align: right;
    }

    .row .v.muted {
      color: var(--muted);
    }

    .empty {
      padding: 28px;
      border: 1px dashed var(--line);
      border-radius: var(--radius-md);
      color: var(--muted);
      text-align: center;
    }

    @keyframes rise {
      from { transform: translateY(12px); opacity: 0; }
      to { transform: translateY(0); opacity: 1; }
    }

    @media (max-width: 1100px) {
      .meta-grid, .summary-grid, .cards {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }
    }

    @media (max-width: 720px) {
      body { padding: 14px; }
      .hero { padding: 18px; }
      .meta-grid, .summary-grid, .cards {
        grid-template-columns: minmax(0, 1fr);
      }
      .toolbar {
        flex-direction: column;
        align-items: flex-start;
      }
      .symbol { font-size: 22px; }
      .row {
        flex-direction: column;
        align-items: flex-start;
      }
      .row .v {
        text-align: left;
      }
    }
  </style>
</head>
<body>
  <main class="shell">
    <section class="hero">
      <div class="eyebrow">
        <span class="dot"></span>
        <span>Realtime Factor Health</span>
      </div>
      <h1>Pnlu Factor Stream</h1>
      <p>
        Symbols stay in <strong>Warming</strong> until a factor appears. Once a factor has appeared,
        a silence longer than the offline threshold is treated as <strong>Offline</strong>.
      </p>
      <div class="meta-grid">
        <article class="panel">
          <div class="label">Profile</div>
          <div class="value" id="meta-profile">-</div>
        </article>
        <article class="panel">
          <div class="label">Endpoint</div>
          <div class="value" id="meta-endpoint">-</div>
        </article>
        <article class="panel">
          <div class="label">Min Periods</div>
          <div class="value" id="meta-min-periods">-</div>
        </article>
        <article class="panel">
          <div class="label">Offline Threshold</div>
          <div class="value" id="meta-offline-after">-</div>
        </article>
      </div>
    </section>

    <section class="summary-grid">
      <article class="stat ok">
        <div class="label">Online</div>
        <div class="value" id="count-online">0</div>
      </article>
      <article class="stat warming">
        <div class="label">Warming</div>
        <div class="value" id="count-warming">0</div>
      </article>
      <article class="stat offline">
        <div class="label">Offline</div>
        <div class="value" id="count-offline">0</div>
      </article>
      <article class="stat">
        <div class="label">Configured Symbols</div>
        <div class="value" id="count-total">0</div>
      </article>
    </section>

    <section>
      <div class="toolbar">
        <h2>Symbols</h2>
        <div class="legend">
          <span class="pill online"><span class="dot"></span>Online</span>
          <span class="pill warming"><span class="dot"></span>Warming</span>
          <span class="pill offline"><span class="dot"></span>Offline</span>
          <span class="pill"><span class="dot"></span><span id="meta-updated">Waiting</span></span>
        </div>
      </div>
      <div class="cards" id="cards"></div>
      <div class="empty" id="empty" hidden>No symbol configured.</div>
    </section>
  </main>

  <script>
    const statusRank = { online: 0, warming: 1, offline: 2 };

    function formatUtc(value) {
      if (!value) return "Never";
      return value;
    }

    function formatAge(seconds) {
      if (seconds == null) return "No data";
      if (seconds < 1) return "just now";
      if (seconds < 60) return `${seconds}s ago`;
      const minutes = Math.floor(seconds / 60);
      const remain = seconds % 60;
      if (minutes < 60) return `${minutes}m ${remain}s ago`;
      const hours = Math.floor(minutes / 60);
      return `${hours}h ${minutes % 60}m ago`;
    }

    function reasonText(item, meta) {
      if (item.status === "online") return "Factor stream is active.";
      if (item.status === "offline") return `Factor had appeared before, but no push for > ${meta.offline_after_seconds}s.`;
      if (item.status === "warming") return `Factor not ready yet. Below min_periods=${meta.min_periods}.`;
      return "Still waiting for the first push from this symbol.";
    }

    function setText(id, value) {
      const node = document.getElementById(id);
      if (node) node.textContent = value;
    }

    function render(snapshot) {
      const meta = snapshot.meta;
      const items = [...snapshot.symbols].sort((a, b) => {
        const left = statusRank[a.status] ?? 9;
        const right = statusRank[b.status] ?? 9;
        if (left !== right) return left - right;
        return a.symbol.localeCompare(b.symbol);
      });

      setText("meta-profile", meta.profile || "custom");
      setText("meta-endpoint", meta.endpoint);
      setText("meta-min-periods", String(meta.min_periods));
      setText("meta-offline-after", `${meta.offline_after_seconds}s`);
      setText("meta-updated", `UI ${snapshot.generated_at_utc}`);
      setText("count-online", String(snapshot.counts.online || 0));
      setText("count-warming", String(snapshot.counts.warming || 0));
      setText("count-offline", String(snapshot.counts.offline || 0));
      setText("count-total", String(snapshot.symbols.length || 0));

      const cards = document.getElementById("cards");
      const empty = document.getElementById("empty");
      cards.innerHTML = "";
      empty.hidden = items.length !== 0;
      cards.hidden = items.length === 0;

      items.forEach((item, index) => {
        const article = document.createElement("article");
        article.className = `card ${item.status}`;
        article.style.animationDelay = `${Math.min(index * 30, 360)}ms`;
        article.innerHTML = `
          <div class="card-head">
            <div>
              <div class="symbol">${item.symbol}</div>
              <div class="caption">${reasonText(item, meta)}</div>
            </div>
            <span class="pill ${item.status}"><span class="dot"></span>${item.status}</span>
          </div>
          <div class="fact">
            <div class="row">
              <div class="k">Last Push</div>
              <div class="v">${formatUtc(item.last_push_utc)}</div>
            </div>
            <div class="row">
              <div class="k">Silence</div>
              <div class="v">${formatAge(item.seconds_since_push)}</div>
            </div>
            <div class="row">
              <div class="k">Last Factor Push</div>
              <div class="v ${item.last_factor_utc ? "" : "muted"}">${formatUtc(item.last_factor_utc)}</div>
            </div>
            <div class="row">
              <div class="k">Target Bucket</div>
              <div class="v ${item.target_ts_utc ? "" : "muted"}">${formatUtc(item.target_ts_utc)}</div>
            </div>
          </div>
        `;
        cards.appendChild(article);
      });
    }

    function connect() {
      const proto = window.location.protocol === "https:" ? "wss" : "ws";
      const ws = new WebSocket(`${proto}://${window.location.host}/ws`);

      ws.onmessage = (event) => {
        try {
          render(JSON.parse(event.data));
        } catch (err) {
          console.error("invalid snapshot", err);
        }
      };

      ws.onclose = () => {
        setText("meta-updated", "Disconnected, retrying...");
        window.setTimeout(connect, 1200);
      };
    }

    fetch("/api/snapshot")
      .then((resp) => resp.json())
      .then(render)
      .catch(() => setText("meta-updated", "Waiting for backend"));

    connect();
  </script>
</body>
</html>
"""


@dataclass
class SymbolRuntime:
    symbol: str
    last_push_monotonic: Optional[float] = None
    last_push_utc: Optional[str] = None
    last_factor_utc: Optional[str] = None
    target_ts_utc: Optional[str] = None
    factor_ready: bool = False
    latest_has_factor: bool = False


@dataclass
class MonitorState:
    symbols: Dict[str, SymbolRuntime]
    profile: Optional[str]
    endpoint: str
    min_periods: int
    offline_after_seconds: int
    topic: str
    started_monotonic: float = field(default_factory=time.monotonic)

    def ensure_symbol(self, symbol: str) -> SymbolRuntime:
        state = self.symbols.get(symbol)
        if state is None:
            state = SymbolRuntime(symbol=symbol)
            self.symbols[symbol] = state
        return state

    def ingest(self, payload: Dict[str, Any]) -> None:
        symbol = str(payload.get("symbol", "")).strip()
        if not symbol:
            return

        now = time.monotonic()
        state = self.ensure_symbol(symbol)
        state.last_push_monotonic = now
        state.last_push_utc = format_ts(payload.get("ts"))
        state.target_ts_utc = format_ts(payload.get("target_ts"))

        has_factor = payload.get("factor") is not None
        state.latest_has_factor = has_factor
        if has_factor:
            state.factor_ready = True
            state.last_factor_utc = state.last_push_utc

    def snapshot(self) -> Dict[str, Any]:
        now = time.monotonic()
        counts = {"online": 0, "warming": 0, "offline": 0}
        rows: List[Dict[str, Any]] = []

        for symbol in sorted(self.symbols):
            state = self.symbols[symbol]
            status, seconds_since_push = compute_status(state, now, self.offline_after_seconds)
            counts[status] += 1
            rows.append(
                {
                    "symbol": symbol,
                    "status": status,
                    "last_push_utc": state.last_push_utc,
                    "last_factor_utc": state.last_factor_utc,
                    "target_ts_utc": state.target_ts_utc,
                    "seconds_since_push": seconds_since_push,
                }
            )

        return {
            "meta": {
                "profile": self.profile,
                "endpoint": self.endpoint,
                "min_periods": self.min_periods,
                "offline_after_seconds": self.offline_after_seconds,
                "topic": self.topic,
            },
            "generated_at_utc": utc_now_text(),
            "counts": counts,
            "symbols": rows,
        }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Monitor pnlu_factor IPC and expose a WS dashboard")
    parser.add_argument("--profile", help="profile name used to derive config paths and IPC endpoint")
    parser.add_argument("--ipc", help="explicit IPC endpoint, e.g. ipc:///tmp/mth_pubs/pnlu_factor/okex-futures-binance-futures.ipc")
    parser.add_argument("--config", help="symbols config path, default derived from profile or ./config.toml")
    parser.add_argument("--pnlu-config", help="pnlu_factor config path, default derived from profile or ./pnlu_factor.toml")
    parser.add_argument("--topic", default="pnlu_factor", help="subscription topic")
    parser.add_argument("--host", default="0.0.0.0", help="HTTP bind host")
    parser.add_argument("--port", type=int, default=8765, help="HTTP bind port")
    parser.add_argument("--offline-after", type=int, default=10, help="mark offline after N seconds without push")
    return parser.parse_args()


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


def infer_profile_from_cwd() -> Optional[str]:
    cwd = Path.cwd()
    for raw in (cwd.name, str(cwd)):
        token = sanitize_profile_token(raw)
        for profile in SUPPORTED_PROFILES:
            if token == profile or token.endswith(f"-{profile}") or raw.endswith(profile):
                return profile
    return None


def endpoint_from_profile(profile: str) -> str:
    return f"ipc:///tmp/mth_pubs/pnlu_factor/{sanitize_profile_token(profile)}.ipc"


def profile_config_path(profile: Optional[str], prefix: str, fallback: str) -> Path:
    if profile:
        candidate = Path(f"{prefix}.{sanitize_profile_token(profile)}.toml")
        if candidate.exists():
            return candidate
    return Path(fallback)


def load_symbols(config_path: Path) -> List[str]:
    if not config_path.exists():
        raise FileNotFoundError(f"symbols config not found: {config_path}")
    with config_path.open("rb") as fh:
        data = tomli.load(fh)
    symbols = data.get("online_symbols") or []
    if not isinstance(symbols, list):
        raise ValueError(f"online_symbols must be an array in {config_path}")
    return [str(item).strip() for item in symbols if str(item).strip()]


def load_min_periods(config_path: Path) -> int:
    if not config_path.exists():
        raise FileNotFoundError(f"pnlu config not found: {config_path}")
    with config_path.open("rb") as fh:
        data = tomli.load(fh)
    section = data.get("pnlu_factor_stream") or {}
    value = section.get("min_periods", 300)
    return max(int(value), 1)


def format_ts(value: Any) -> Optional[str]:
    if value is None:
        return None
    try:
        seconds = int(value)
    except Exception:
        text = str(value).strip()
        return text or None
    return datetime.fromtimestamp(seconds, tz=timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def utc_now_text() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")


def compute_status(
    state: SymbolRuntime,
    now: float,
    offline_after_seconds: int,
) -> tuple[str, Optional[int]]:
    if state.last_push_monotonic is None:
        return "warming", None

    silence = max(0, int(now - state.last_push_monotonic))
    if state.factor_ready and silence > offline_after_seconds:
        return "offline", silence
    if state.latest_has_factor:
        return "online", silence
    return "warming", silence


async def index(_request: web.Request) -> web.Response:
    return web.Response(text=HTML_PAGE, content_type="text/html")


async def api_snapshot(request: web.Request) -> web.Response:
    state: MonitorState = request.app["monitor_state"]
    return web.json_response(state.snapshot())


async def websocket_handler(request: web.Request) -> web.WebSocketResponse:
    ws = web.WebSocketResponse(heartbeat=20)
    await ws.prepare(request)
    clients: Set[web.WebSocketResponse] = request.app["ws_clients"]
    clients.add(ws)
    state: MonitorState = request.app["monitor_state"]
    await ws.send_json(state.snapshot())

    try:
        async for msg in ws:
            if msg.type == WSMsgType.ERROR:
                break
    finally:
        clients.discard(ws)

    return ws


async def publish_snapshots(app: web.Application) -> None:
    try:
        while True:
            payload = json.dumps(app["monitor_state"].snapshot(), ensure_ascii=True)
            closed: List[web.WebSocketResponse] = []
            for ws in list(app["ws_clients"]):
                if ws.closed:
                    closed.append(ws)
                    continue
                try:
                    await ws.send_str(payload)
                except Exception:
                    closed.append(ws)
            for ws in closed:
                app["ws_clients"].discard(ws)
            await asyncio.sleep(1)
    except asyncio.CancelledError:
        raise


async def watch_ipc(app: web.Application) -> None:
    ctx = zmq.asyncio.Context.instance()
    socket = ctx.socket(zmq.SUB)
    topic = app["monitor_state"].topic.encode("utf-8")
    socket.setsockopt(zmq.SUBSCRIBE, topic)
    socket.connect(app["monitor_state"].endpoint)

    try:
        while True:
            parts = await socket.recv_multipart()
            if len(parts) < 2:
                continue
            body = parts[1]
            try:
                payload = json.loads(body.decode("utf-8", "ignore"))
            except Exception:
                continue
            app["monitor_state"].ingest(payload)
    except asyncio.CancelledError:
        raise
    finally:
        socket.close(0)


async def start_background(app: web.Application) -> None:
    app["ipc_task"] = asyncio.create_task(watch_ipc(app))
    app["publish_task"] = asyncio.create_task(publish_snapshots(app))


async def stop_background(app: web.Application) -> None:
    for key in ("ipc_task", "publish_task"):
        task = app.get(key)
        if task is None:
            continue
        task.cancel()
        try:
            await task
        except asyncio.CancelledError:
            pass

    for ws in list(app["ws_clients"]):
        await ws.close()


def build_app(state: MonitorState) -> web.Application:
    app = web.Application()
    app["monitor_state"] = state
    app["ws_clients"] = set()
    app.router.add_get("/", index)
    app.router.add_get("/api/snapshot", api_snapshot)
    app.router.add_get("/ws", websocket_handler)
    app.on_startup.append(start_background)
    app.on_shutdown.append(stop_background)
    return app


def resolve_paths(args: argparse.Namespace, profile: Optional[str]) -> tuple[Path, Path]:
    symbols_path = Path(args.config) if args.config else profile_config_path(profile, "config", "config.toml")
    pnlu_path = Path(args.pnlu_config) if args.pnlu_config else profile_config_path(profile, "pnlu_factor", "pnlu_factor.toml")
    return symbols_path, pnlu_path


def install_signal_logs() -> None:
    for sig in (signal.SIGINT, signal.SIGTERM):
        signal.signal(sig, lambda _signo, _frame: None)


def main() -> int:
    args = parse_args()
    profile = args.profile or infer_profile_from_cwd()
    endpoint = args.ipc or (endpoint_from_profile(profile) if profile else "")
    if not endpoint:
        raise SystemExit("IPC endpoint is required (--ipc or --profile, or run inside a profile dir)")

    symbols_path, pnlu_path = resolve_paths(args, profile)
    symbols = load_symbols(symbols_path)
    min_periods = load_min_periods(pnlu_path)

    state = MonitorState(
        symbols={symbol: SymbolRuntime(symbol=symbol) for symbol in symbols},
        profile=profile,
        endpoint=endpoint,
        min_periods=min_periods,
        offline_after_seconds=max(args.offline_after, 1),
        topic=args.topic,
    )
    app = build_app(state)

    install_signal_logs()
    print(f"HTTP: http://{args.host}:{args.port}")
    print(f"Profile: {profile or 'custom'}")
    print(f"IPC: {endpoint}")
    print(f"Symbols: {len(symbols)} from {symbols_path}")
    print(f"min_periods: {min_periods} from {pnlu_path}")
    web.run_app(app, host=args.host, port=args.port, handle_signals=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
