# CPU to flush coalesced craft-progress notifications

With zero subscribers, a 472-row flush fell from **35.53 µs to 0.061 µs**
of producer CPU. The candidate won all 24 pairs at every nonempty size.
The interested path showed no consistent slowdown or speedup across sizes.

Median **ns per flush** over 24 alternating-order release pairs on Linux
CPU 8 (Ryzen 9 9950X3D, rustc 1.100.0, opt-level 3, LTO off).
[Raw paired samples](samples.csv).

| Rows | No subscribers: baseline → candidate | Matching player: baseline → candidate |
|---:|---:|---:|
| 0 | 2.15 → 2.16 | 2.15 → 2.25 |
| 1 | 82.62 → 3.85 | 208.65 → 209.62 |
| 2 | 154.12 → 3.99 | 397.38 → 404.01 |
| 144 | 10,988.94 → 21.63 | 27,454.55 → 27,016.68 |
| 472 | 35,530.17 → 61.35 | 92,696.70 → 89,100.96 |
| 538 | 40,336.45 → 67.46 | 104,811.98 → 102,433.53 |

[Replay](replay.rs) calls the production `crafting::notifications::progress`
on prefilled maps. Both release binaries run the same harness. Rows are
synthetic: valid microsecond timestamps, one owner, distinct crafts. Map
sizes use the [live progressive writes](../live-progressive-writes/README.md)
measurements: 1 and 2 rows in a small region, 144 and 472 for the floor and
median of a nine-region sample, and 538 for the later median. These are
representative coalesced sizes, not a capture of the notification maps.

The zero-subscription case exercises the producer with no audience. The
matching case subscribes one player to **every** delta, runs normal message
construction and routing checks, and enqueues the messages. The harness
checks the expected number of outbound messages, empty maps, and no database
lookups. A separate worker test covers delivery to a client subscribing while
two updates to one craft are pending. Interest is sampled after persistence;
a subscription starting after that check can begin with the next flush. Any
existing subscription, including one to a different topic, retains the original
formatting and filtering path.

The clock measures Linux thread CPU on CPU 8. Each sample times 32 groups
after one warm-up group, each with up to 1,024 flushes or 4,096 rows. Map
refill, receiver drain, SQL writes, and socket fanout are outside the timer.
There is no claim subscriber in either arm. This isolates producer CPU;
it does not measure persistence latency, process-wide CPU, or passive crafts.

## Reproduce

From `rust/api-server` in a **disposable copy** of the source tree containing
this replay, on Linux, using Bash:

```bash
tmp=$(mktemp -d)
bench=api/benches/craft-notification-interest
export CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_DEBUG=0
build() {
  cargo test --release --locked -p api --lib \
    --features tokio-worker-queues,spacetimedb-tokio-websockets-rustls \
    --no-run --message-format=json > "$tmp/build.json" || return
  cp "$(python3 -c 'import json, sys
print(next(x["executable"] for l in open(sys.argv[1]) if (x:=json.loads(l)).get("executable") and x["target"]["name"] == "api"))' "$tmp/build.json")" "$tmp/$1"
}
build candidate
git show a5344722:rust/api-server/api/src/crafting/notifications.rs > api/src/crafting/notifications.rs
cat >> api/src/crafting/notifications.rs <<'RS'
#[cfg(all(test, target_os = "linux"))]
#[path = "../../benches/craft-notification-interest/replay.rs"]
mod replay;
RS
build baseline
python3 "$bench/compare.py" "$tmp/baseline" "$tmp/candidate" > "$tmp/samples.csv"
```
