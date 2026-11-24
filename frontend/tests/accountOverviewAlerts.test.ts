import { expect, test } from "bun:test";
import {
  alertsAllowed,
  passiveReadyAlerts,
  vitalAlerts,
  IdleAlertTimer,
  FoodExpiryTracker,
  type AlertPreferences,
} from "../app/utils/accountOverviewAlerts";
import type { PlayerCraft } from "../app/types/PlayerCraft";
import type { StatsSnapshot } from "../app/types/StatsSnapshot";
import {
  emptyJournal,
  pruneJournal,
  recordObservation,
} from "../app/utils/accountOverviewAlertJournal";
import { currentPlayerActivity } from "../app/utils/playerActivity";

test("global, player and block mutes take precedence without muting other scopes", () => {
  const prefs: AlertPreferences = {
    enabled: true,
    volume: 0,
    enabledTypes: ["low-health", "activity-stopped"],
    mutedPlayers: ["a"],
    mutedBlocks: { b: ["stats"] },
  };
  expect(alertsAllowed(prefs, "a", "activity-stopped")).toBe(false);
  expect(alertsAllowed(prefs, "b", "low-health")).toBe(false);
  expect(alertsAllowed(prefs, "b", "activity-stopped")).toBe(true);
  expect(alertsAllowed(prefs, "c", "low-health")).toBe(true);
  expect(alertsAllowed(prefs, "c", "full-stamina")).toBe(false);
  expect(alertsAllowed({ ...prefs, enabled: false }, "c", "low-health")).toBe(false);
});

const queued: PlayerCraft = {
  id: "1",
  owner_id: "42",
  kind: "Passive",
  recipe_id: 123,
  recipe_name: "Bricks",
  status: "Queued",
  quantity: 1,
  public: true,
  progress: null,
  progress_total: null,
  slot: null,
  timestamp: null,
  duration_seconds: null,
  outputs: [],
  building_id: "1",
  building_name: "Kiln",
  building_context: { claim_id: "2", claim_name: "Home" },
};

test("craft alerts ignore baseline, progress ticks, other blocks and excluded claims", () => {
  const done = { ...queued, status: "Complete" };
  expect(passiveReadyAlerts(null, [done], [])).toEqual([]);
  expect(passiveReadyAlerts([queued], [{ ...queued, progress: 1 }], [])).toEqual([]);
  expect(passiveReadyAlerts([queued], [{ ...done, kind: "Active" }], [])).toEqual([]);
  expect(passiveReadyAlerts([queued], [done], ["unknown"])).toEqual([]);
  expect(passiveReadyAlerts([queued], [done], ["claim:2"])).toMatchObject([
    { type: "passive-ready", eventId: "1" },
  ]);
  expect(passiveReadyAlerts([done], [done], [])).toEqual([]);
  expect(passiveReadyAlerts([done], [], [])).toEqual([]);
});

const stats: StatsSnapshot = {
  player_id: "1",
  available: true,
  health: 26,
  max_health: 100,
  stamina: 99,
  max_stamina: 100,
  buffs: [],
  buffs_complete: true,
  food_metadata_available: true,
};
const player = { id: "1", name: "Sky" };

test("vital alerts rearm after recovery and do not replay after reloading", () => {
  let journal = emptyJournal();
  const observe = (snapshot: StatsSnapshot) =>
    vitalAlerts(snapshot).flatMap((e) => recordObservation(journal, player, e, true, 1000) ?? []);
  const lowAndFull = { ...stats, health: 25, stamina: 100 };
  expect(observe(lowAndFull)).toHaveLength(0); // Initial snapshot
  expect(observe(stats)).toHaveLength(0);
  expect(observe(lowAndFull).map((e) => e.type)).toEqual(["low-health", "full-stamina"]);
  journal = JSON.parse(JSON.stringify(journal));
  expect(observe({ ...stats, health: null, stamina: null })).toHaveLength(0);
  expect(observe({ ...lowAndFull, health: 12 })).toHaveLength(0);
  observe(stats);
  expect(observe(lowAndFull)).toHaveLength(2);
});

test("stamina refills during work or a running regen buff are consumed, not delayed", () => {
  const regen = {
    buff_id: 7,
    start_timestamp: 10,
    duration: 30,
    remaining_seconds: 5n,
    paused: false,
    is_food_associated: false,
    name: "Tonic",
    icon_asset_name: null,
    description: null,
    values: [],
    stats: [{ name: "ActiveStaminaRegenRate", value: 1, is_percent: false }],
  };
  const full = { ...stats, stamina: 100, health: 20 };
  for (const activity of ["craft", "gather", null] as const) {
    const j = emptyJournal();
    const observe = (s: StatsSnapshot, kind: typeof activity = null) =>
      vitalAlerts(s, kind)
        .flatMap((e) => recordObservation(j, player, e, true, 1000) ?? [])
        .map((e) => e.type);
    observe(stats);
    expect(observe({ ...full, buffs: activity ? [] : [regen] }, activity)).toEqual(["low-health"]);
    expect(observe(full)).toEqual([]);
    observe(stats);
    expect(observe(full)).toContain("full-stamina");
  }
  function suppressed(buff: NonNullable<StatsSnapshot["buffs"]>[number]) {
    return vitalAlerts({ ...full, buffs: [buff] }).find((e) => e.type === "full-stamina")
      ?.suppressed;
  }
  expect(suppressed({ ...regen, paused: true })).toBe(false);
  expect(suppressed({ ...regen, remaining_seconds: 0n })).toBe(false);
  expect(
    suppressed({
      ...regen,
      remaining_seconds: null,
      stats: [{ name: "PassiveStaminaRegenRate", value: 2, is_percent: true }],
    }),
  ).toBe(true);
  expect(
    suppressed({ ...regen, stats: [{ name: "MaxStamina", value: 20, is_percent: false }] }),
  ).toBe(false);
  expect(
    suppressed({
      ...regen,
      stats: [{ name: "PassiveStaminaRegenRate", value: -2, is_percent: false }],
    }),
  ).toBe(false);
});

test("only uninterrupted idle following crafting or gathering fires, once, at 20 seconds", () => {
  const timer = new IdleAlertTimer();
  const idle = { ...currentPlayerActivity(null, 0), label: "Idle" };
  const walk = { ...idle, label: "Walking", actionStart: 1 };
  for (const kind of ["craft", "gather"] as const) {
    const active = {
      ...idle,
      label: kind === "craft" ? "Crafting" : "Gathering",
      actionStart: 123,
      recipeKind: kind,
    };
    timer.update(active, 0);
    timer.update(idle, 1000);
    expect(timer.update(idle, 20999)).toBeUndefined();
    expect(timer.update(idle, 21000)).toMatchObject({ type: "activity-stopped", eventId: "123" });
    expect(timer.update(idle, 60000)).toBeUndefined();
    for (const interruption of [
      walk,
      null,
      { ...idle, label: "Offline" },
      { ...idle, label: "Activity unavailable" },
    ]) {
      timer.update(active, 0);
      timer.update(idle, 500);
      timer.update(interruption, 3000);
      timer.update(idle, 4500);
      expect(timer.update(idle, 30000)).toBeUndefined();
    }
  }
});

test("idle after exhaustion waits for a full refill and a fresh 20 seconds", () => {
  for (const kind of ["craft", "gather"] as const) {
    const timer = new IdleAlertTimer();
    const idle = { ...currentPlayerActivity(null, 0), label: "Idle" };
    const work = { ...idle, label: "Working", recipeKind: kind, actionStart: 123 };
    const tick = (activity: typeof idle, now: number, stamina: number | null, cost = 5) =>
      timer.update(activity, now, { available: true, stamina, max_stamina: 100 }, cost);
    tick(work, 0, 2); // Insufficient for the next hit, even if it hasn't reached zero.
    tick(idle, 1000, 10);
    expect(tick(idle, 30000, null)).toBeUndefined();
    expect(tick(idle, 60000, 99)).toBeUndefined();
    expect(tick(idle, 62000, 100)).toBeUndefined();
    expect(tick(idle, 81999, 100)).toBeUndefined();
    expect(tick(idle, 82000, 100)).toMatchObject({ eventId: "123" });
    expect(tick(idle, 150000, 100)).toBeUndefined();

    tick(work, 151000, 50, 0);
    tick(idle, 152000, 45, 0);
    tick(idle, 153000, 0, 0); // Stats arrived after idle; recipe cost unknown.
    expect(tick(idle, 185000, 30, 0)).toBeUndefined();
    const resumed = { ...work, actionStart: 456 };
    tick(resumed, 186000, 40);
    tick(idle, 187000, 35); // Enough stamina: ordinary stop doesn't wait for full.
    expect(tick(idle, 207000, 50)).toMatchObject({ eventId: "456" });
  }
});

test("timed food buffs expire once; paused, permanent and replaced timers don't fire", () => {
  const tracker = new FoodExpiryTracker();
  const buff = {
    buff_id: 11,
    start_timestamp: 10,
    remaining_seconds: 5n,
    paused: false,
    is_food_associated: true,
    name: "Soup",
    stats: null,
    values: [],
    duration: 61,
    icon_asset_name: null,
    description: null,
  };
  tracker.snapshot({ ...stats, buffs: [buff] }, 0);
  expect(tracker.tick(4999)).toEqual([]);
  expect(tracker.tick(5000)).toMatchObject([{ type: "food-expired", eventId: "11:10" }]);
  expect(tracker.snapshot(stats, 5010)).toEqual([]);
  tracker.snapshot({ ...stats, buffs: [buff] }, 0);
  tracker.snapshot({ ...stats, buffs: [{ ...buff, paused: true }] }, 2000);
  expect(tracker.tick(10000)).toEqual([]);
  tracker.snapshot({ ...stats, buffs: [{ ...buff, remaining_seconds: null }] }, 0);
  expect(tracker.tick(10000)).toEqual([]);
  tracker.snapshot({ ...stats, buffs: [buff] }, 0);
  expect(tracker.snapshot(stats, 1500)).toEqual([]);
  tracker.snapshot({ ...stats, buffs: [buff] }, 0);
  expect(tracker.snapshot(stats, 5000)).toHaveLength(1);
  expect(tracker.tick(5100)).toEqual([]);
  for (const duration of [10, 60]) {
    const short = { ...stats, buffs: [{ ...buff, duration }] };
    tracker.snapshot(short, 0);
    expect(tracker.tick(5000)).toEqual([]);
    tracker.snapshot(short, 0);
    expect(tracker.snapshot(stats, 5000)).toEqual([]);
  }
});

test("history bounds, manual clearing, and delivery receipts are independent", () => {
  const j = emptyJournal();
  const obs = { type: "passive-ready", eventId: "p" } as const;
  expect(recordObservation(j, player, obs, false, 0)).toBeUndefined();
  expect(recordObservation(j, player, obs, true, 1)).toBeUndefined();
  for (let i = 0; i < 205; i++)
    recordObservation(j, player, { ...obs, eventId: String(i) }, true, i);
  expect(j.entries).toHaveLength(200);
  expect(j.entries[0]?.time).toBe(204);
  const day = 86400000;
  pruneJournal(j, 8 * day);
  expect(j.entries).toHaveLength(0);
  expect(recordObservation(j, player, { ...obs, eventId: "204" }, true, 8 * day)).toBeUndefined();
  expect(
    recordObservation(j, { id: "2", name: "Other" }, { ...obs, eventId: "204" }, true, 8 * day),
  ).toBeDefined();
  j.entries = [];
  expect(
    recordObservation(j, { id: "2", name: "Other" }, { ...obs, eventId: "204" }, true, 9 * day),
  ).toBeUndefined();
  pruneJournal(j, 40 * day);
  expect(Object.keys(j.marks)).toHaveLength(0);
});
