#!/usr/bin/env python3
"""Summarize worker-message metrics without dumping the Prometheus response.

Uses only Python's standard library. Example:
    python3 scripts/worker_metrics.py --samples 3 --interval 5

Estimated queued messages = sent - send errors - received. Received means taken
from the input channel, NOT processed/persisted. Initial snapshots count as one
message regardless of their row count. This cannot measure database-write queues
or queued bytes; neither is exposed by these metrics.
"""

import argparse
from datetime import datetime, timezone
import json
import math
import re
import sys
import time
import urllib.error
import urllib.request


METRICS = {
    "worker_queue_sent_total": "sent",
    "worker_queue_received_total": "received",
    "worker_queue_send_errors_total": "errors",
    "worker_queue_receive_batch_size": "last_batch",
    "worker_initial_batches_sent_total": "initial_sent",
    "worker_initial_batches_received_total": "initial_received",
    "worker_initial_batches_send_errors_total": "initial_errors",
    "worker_initial_batches_discarded_total": "initial_discarded",
    "worker_initial_snapshots_completed_total": "initial_end",
}
PREFIXES = tuple(name + "{" for name in METRICS)
SAMPLE = re.compile(r"^(\w+)\{(.*)\}\s+(\S+)(?:\s+.*)?$")
WORKER = re.compile(r'(?:^|,)\s*worker="((?:\\.|[^"\\])*)"')


def parse_metrics(lines, process=None):
    workers = {}
    for line in lines:
        if process is not None and line.startswith("process_resident_memory_bytes "):
            try:
                value = float(line.split()[1])
                if math.isfinite(value) and value >= 0:
                    process["rss"] = value
            except (ValueError, IndexError):
                pass
            continue
        # Discard unrelated metrics before parsing labels or allocating records.
        if not line.startswith(PREFIXES):
            continue
        sample = SAMPLE.match(line.rstrip())
        if not sample or sample[1] not in METRICS:
            continue
        label = WORKER.search(sample[2])
        if not label:
            continue
        try:
            worker = json.loads('"' + label[1] + '"')
            value = float(sample[3])
        except ValueError:
            continue
        if not math.isfinite(value):
            continue
        values = workers.setdefault(worker, {})
        key = METRICS[sample[1]]
        if key == "last_batch":
            values[key] = max(values.get(key, 0), value)
        else:
            values[key] = values.get(key, 0) + value
    return workers


def scrape(url, timeout, process=None):
    request = urllib.request.Request(url, headers={"Accept": "text/plain"})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        # Stream lines: never retain or print the complete response.
        return parse_metrics((line.decode("utf-8") for line in response), process)


def queued(values):
    if "sent" not in values or "received" not in values:
        return None
    return values["sent"] - values.get("errors", 0) - values["received"]


def number(value):
    return "-" if value is None else f"{value:,.0f}"


def rate(current, previous, key, elapsed):
    if not previous or key not in current or key not in previous:
        return "-"
    delta = current[key] - previous[key]
    return "reset" if delta < 0 else f"{delta / elapsed:.1f}"


def report(workers, previous=None, elapsed=None, rss=None):
    print(datetime.now(timezone.utc).isoformat(timespec="seconds"))
    if rss is not None:
        print(f"Process RSS: {rss / 1048576:.1f} MiB ({rss / 1e9:.3f} GB)")
    print(f"{'worker':32} {'queued~':>10} {'sent':>13} {'received':>13} {'errors':>8} {'batch*':>7} {'sent/s':>8} {'recv/s':>8}")
    for worker, values in sorted(
        workers.items(), key=lambda entry: (-(queued(entry[1]) or 0), entry[0])
    ):
        old = previous.get(worker) if previous is not None else None
        estimate = queued(values)
        backlog = "!labels" if estimate is not None and estimate < 0 else number(estimate)
        print(
            f"{worker:32} {backlog:>10} "
            f"{number(values.get('sent')):>13} {number(values.get('received')):>13} "
            f"{number(values.get('errors', 0)):>8} {number(values.get('last_batch')):>7} "
            f"{rate(values, old, 'sent', elapsed):>8} "
            f"{rate(values, old, 'received', elapsed):>8}"
        )
    print("queued~ = sent - errors - received; approximate, not in-flight work or bytes.")
    print("batch* = last receive batch (not snapshot rows or database batch size); '-' = unavailable.")
    if any(queued(values) is not None and queued(values) < 0 for values in workers.values()):
        print("WARNING: !labels = received exceeds successful sends; check worker labels/counter resets. Tiny differences may be scrape races.")
    initial = {name: values for name, values in workers.items() if "initial_sent" in values or "initial_end" in values}
    if initial:
        print("Initial snapshot subqueues (batches, not rows/bytes):")
        print(f"{'worker':32} {'queued~':>10} {'sent':>10} {'received':>10} {'discarded':>10} {'end read':>10}")
        for name, values in sorted(initial.items()):
            pending = values.get("initial_sent", 0) - values.get("initial_errors", 0) - values.get("initial_received", 0) - values.get("initial_discarded", 0)
            print(f"{name:32} {number(pending):>10} {number(values.get('initial_sent', 0)):>10} {number(values.get('initial_received', 0)):>10} {number(values.get('initial_discarded', 0)):>10} {number(values.get('initial_end', 0)):>10}")
        print("end read = complete input received, not necessarily database persistence finished.")
    print(flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="http://localhost:8000/metrics")
    parser.add_argument("--samples", type=int, default=1)
    parser.add_argument("--interval", type=float, default=5)
    parser.add_argument("--timeout", type=float, default=10)
    parser.add_argument("--rss", action="store_true", help="Also print process RSS if exposed by the server")
    parser.add_argument("--worker", action="append", help="Show only this worker; repeat for multiple workers")
    args = parser.parse_args()
    if args.samples < 1 or not math.isfinite(args.interval) or args.interval <= 0 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("samples must be >= 1; interval and timeout must be finite and > 0")
    previous = None
    previous_time = None
    try:
        for sample in range(args.samples):
            if sample:
                time.sleep(args.interval)
            process = {} if args.rss else None
            workers = scrape(args.url, args.timeout, process)
            now = time.monotonic()
            if args.worker:
                workers = {name: values for name, values in workers.items() if name in args.worker}
            if not workers:
                print("No matching worker metrics found (workers may not have received messages yet).", file=sys.stderr)
                return 1
            report(workers, previous, now - previous_time if previous_time is not None else None,
                   rss=process.get("rss") if process is not None else None)
            if args.rss and "rss" not in process:
                print("Process RSS metric unavailable.", file=sys.stderr)
            previous, previous_time = workers, now
    except (OSError, urllib.error.URLError, UnicodeError) as error:
        print(f"Unable to read worker metrics: {error}", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
