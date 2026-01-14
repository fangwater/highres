import argparse
import csv
from collections import defaultdict, deque
from dataclasses import dataclass
from typing import Optional, Tuple


@dataclass
class OpenAgg:
    cts_sec: int
    uts_sec: int
    price: float
    side: str
    order_range: Optional[float]
    tlen_notional: float
    famount_sum: float


@dataclass
class CloseAgg:
    fts_sec: int
    close_range: Optional[float]
    close_count: int
    camount_sum: float
    pa_sum: float


def parse_open_range(oid: str) -> Optional[float]:
    parts = oid.split("_")
    if len(parts) <= 2:
        return None
    try:
        return round(float(parts[2]) * 10000, 0)
    except ValueError:
        return None


def parse_close_range(oid: str) -> Optional[float]:
    parts = oid.split("_")
    if len(parts) <= 5:
        return None
    try:
        return round(float(parts[5]) * 10000, 0)
    except ValueError:
        return None


def parse_close_fkey(raw_fkey: str) -> str:
    parts = raw_fkey.split("_")
    return parts[2] if len(parts) >= 3 else raw_fkey


def stream_orders(
    input_path: str,
    output_path: str,
    period_interval: int,
    window_period: int,
    shift_period: int,
    rolling_window: Optional[int],
    min_periods: int,
    compute_rolling: bool,
) -> None:
    if period_interval <= 0:
        raise ValueError("period_interval must be > 0")
    if window_period <= 0:
        raise ValueError("window_period must be > 0")
    if window_period % period_interval != 0:
        raise ValueError("window_period must be a multiple of period_interval")
    if shift_period < 0:
        raise ValueError("shift_period must be >= 0")

    shift_seconds = shift_period * period_interval
    window_buckets = window_period // period_interval
    if compute_rolling:
        if rolling_window is None:
            rolling_window = window_buckets
        if rolling_window <= 0:
            raise ValueError("rolling_window must be > 0")
        if min_periods > rolling_window:
            min_periods = rolling_window

    open_by_fkey = {}
    close_by_fkey = {}
    open_buckets = defaultdict(set)
    close_buckets = defaultdict(set)
    open_bucket_order = deque()
    close_bucket_order = deque()
    open_bucket_seen = set()
    close_bucket_seen = set()

    roll_values = deque()
    roll_sum = 0.0
    roll_count = 0
    last_label = None

    open_fkeys_seen = 0
    close_fkeys_seen = 0
    matched_total = 0
    dropped_total = 0

    def add_open_bucket(bucket_ts: int, fkey: str) -> None:
        if bucket_ts not in open_buckets:
            open_buckets[bucket_ts] = set()
        open_buckets[bucket_ts].add(fkey)
        if bucket_ts not in open_bucket_seen:
            open_bucket_seen.add(bucket_ts)
            open_bucket_order.append(bucket_ts)

    def add_close_bucket(bucket_ts: int, fkey: str) -> None:
        if bucket_ts not in close_buckets:
            close_buckets[bucket_ts] = set()
        close_buckets[bucket_ts].add(fkey)
        if bucket_ts not in close_bucket_seen:
            close_bucket_seen.add(bucket_ts)
            close_bucket_order.append(bucket_ts)

    def expire_old(current_tp: int) -> None:
        expire_before = current_tp - window_period
        while open_bucket_order and open_bucket_order[0] < expire_before:
            bucket_ts = open_bucket_order.popleft()
            open_bucket_seen.discard(bucket_ts)
            fkeys = open_buckets.pop(bucket_ts, None)
            if fkeys:
                for fkey in fkeys:
                    open_by_fkey.pop(fkey, None)
        while close_bucket_order and close_bucket_order[0] < expire_before:
            bucket_ts = close_bucket_order.popleft()
            close_bucket_seen.discard(bucket_ts)
            fkeys = close_buckets.pop(bucket_ts, None)
            if fkeys:
                for fkey in fkeys:
                    close_by_fkey.pop(fkey, None)

    def update_rolling(value: Optional[float]) -> Optional[float]:
        nonlocal roll_sum, roll_count
        if not compute_rolling:
            return None
        roll_values.append(value)
        if value is not None:
            roll_sum += value
            roll_count += 1
        if len(roll_values) > rolling_window:
            old = roll_values.popleft()
            if old is not None:
                roll_sum -= old
                roll_count -= 1
        if roll_count >= min_periods:
            return roll_sum / roll_count if roll_count else None
        return None

    def finalize_bucket(target_ts: int) -> Tuple[Optional[float], int]:
        nonlocal matched_total, dropped_total
        if target_ts not in open_buckets:
            return None, 0
        pnlu_sum = 0.0
        pnlu_count = 0
        fkeys = open_buckets.pop(target_ts, set())
        for fkey in fkeys:
            open_rec = open_by_fkey.get(fkey)
            close_rec = close_by_fkey.get(fkey)
            if not open_rec or not close_rec or close_rec.camount_sum <= 0:
                dropped_total += 1
                open_by_fkey.pop(fkey, None)
                continue
            cprice = close_rec.pa_sum / close_rec.camount_sum
            if open_rec.price <= 0:
                dropped_total += 1
                open_by_fkey.pop(fkey, None)
                close_by_fkey.pop(fkey, None)
                continue
            if open_rec.side == "buy":
                pnlu = (cprice - open_rec.price) / open_rec.price
            else:
                pnlu = (open_rec.price - cprice) / open_rec.price
            pnlu_sum += pnlu
            pnlu_count += 1
            matched_total += 1
            open_by_fkey.pop(fkey, None)
            close_by_fkey.pop(fkey, None)
        return (pnlu_sum if pnlu_count > 0 else None), pnlu_count

    last_tp = None

    with open(input_path, newline="") as in_file, open(output_path, "w", newline="") as out_file:
        reader = csv.reader(in_file)
        writer = csv.writer(out_file)
        if compute_rolling:
            writer.writerow(["ts", "target_ts", "pnlu_sum", "pnlu_count", "ylabel"])
        else:
            writer.writerow(["ts", "target_ts", "pnlu_sum", "pnlu_count"])

        for row in reader:
            if len(row) < 17:
                continue
            try:
                oid = row[0]
                cts_sec = int(int(row[1]) / 1000)
                uts_sec = int(int(row[2]) / 1000)
                sid = int(row[6])
                side = row[7].lower()
                price = float(row[8])
                famount = float(row[10])
                tlen = float(row[13])
                raw_fkey = row[14]
            except (ValueError, IndexError):
                continue

            current_tp = (uts_sec // period_interval) * period_interval
            if last_tp is None:
                last_tp = current_tp - period_interval
            while current_tp > last_tp:
                last_tp += period_interval
                expire_old(last_tp)
                target_ts = last_tp - shift_seconds
                pnlu_value, pnlu_count = finalize_bucket(target_ts)
                mean = update_rolling(pnlu_value)
                if compute_rolling:
                    ylabel = mean if mean is not None else last_label
                    if mean is not None:
                        last_label = mean
                    writer.writerow([
                        last_tp,
                        target_ts,
                        "" if pnlu_value is None else pnlu_value,
                        pnlu_count,
                        "" if ylabel is None else ylabel,
                    ])
                else:
                    writer.writerow([
                        last_tp,
                        target_ts,
                        "" if pnlu_value is None else pnlu_value,
                        pnlu_count,
                    ])

            if oid.startswith("o") and sid == 0:
                open_range = parse_open_range(oid)
                open_tp = (cts_sec // period_interval) * period_interval
                rec = open_by_fkey.get(raw_fkey)
                if rec is None:
                    open_by_fkey[raw_fkey] = OpenAgg(
                        cts_sec=cts_sec,
                        uts_sec=uts_sec,
                        price=price,
                        side=side,
                        order_range=open_range,
                        tlen_notional=tlen * price,
                        famount_sum=famount,
                    )
                    add_open_bucket(open_tp, raw_fkey)
                    open_fkeys_seen += 1
                else:
                    rec.uts_sec = uts_sec
                    rec.price = price
                    rec.side = side
                    rec.order_range = open_range
                    rec.tlen_notional = tlen * price
                    rec.famount_sum += famount
            elif oid.startswith("c") and sid == 1:
                close_fkey = parse_close_fkey(raw_fkey)
                close_tp = (uts_sec // period_interval) * period_interval
                close_range = parse_close_range(oid)
                rec = close_by_fkey.get(close_fkey)
                if rec is None:
                    close_by_fkey[close_fkey] = CloseAgg(
                        fts_sec=uts_sec,
                        close_range=close_range,
                        close_count=1,
                        camount_sum=famount,
                        pa_sum=famount * price,
                    )
                    add_close_bucket(close_tp, close_fkey)
                    close_fkeys_seen += 1
                else:
                    rec.fts_sec = uts_sec
                    rec.close_range = close_range
                    rec.close_count += 1
                    rec.camount_sum += famount
                    rec.pa_sum += famount * price

        if last_tp is not None:
            for _ in range(shift_period):
                last_tp += period_interval
                expire_old(last_tp)
                target_ts = last_tp - shift_seconds
                pnlu_value, pnlu_count = finalize_bucket(target_ts)
                mean = update_rolling(pnlu_value)
                if compute_rolling:
                    ylabel = mean if mean is not None else last_label
                    if mean is not None:
                        last_label = mean
                    writer.writerow([
                        last_tp,
                        target_ts,
                        "" if pnlu_value is None else pnlu_value,
                        pnlu_count,
                        "" if ylabel is None else ylabel,
                    ])
                else:
                    writer.writerow([
                        last_tp,
                        target_ts,
                        "" if pnlu_value is None else pnlu_value,
                        pnlu_count,
                    ])

    print(
        "open_fkeys=%d close_fkeys=%d matched=%d dropped=%d"
        % (open_fkeys_seen, close_fkeys_seen, matched_total, dropped_total)
    )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Stream order matching and factor calculation (5s period by default)."
    )
    parser.add_argument("--input", required=True, help="Path to orders CSV.")
    parser.add_argument(
        "--output",
        default=None,
        help="Output CSV path. Default: <input>_stream_factor.csv",
    )
    parser.add_argument("--period", type=int, default=5, help="Period interval (sec).")
    parser.add_argument("--window", type=int, default=1800, help="Window period (sec).")
    parser.add_argument("--shift", type=int, default=120, help="Shift period (in buckets).")
    parser.add_argument(
        "--rolling-window",
        type=int,
        default=None,
        help="Rolling window size (in buckets). Default = window/period.",
    )
    parser.add_argument("--min-periods", type=int, default=300, help="Min periods for rolling mean.")
    parser.add_argument(
        "--no-rolling",
        action="store_true",
        help="Only output pnlu_sum/pnlu_count without rolling label.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    output_path = args.output
    if output_path is None:
        output_path = args.input.rsplit(".", 1)[0] + "_stream_factor.csv"
    stream_orders(
        input_path=args.input,
        output_path=output_path,
        period_interval=args.period,
        window_period=args.window,
        shift_period=args.shift,
        rolling_window=args.rolling_window,
        min_periods=args.min_periods,
        compute_rolling=not args.no_rolling,
    )


if __name__ == "__main__":
    main()
