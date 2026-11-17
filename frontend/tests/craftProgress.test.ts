import { expect, test } from "bun:test";
import { applyCraftProgress, applyPassiveCraftUpdate } from "../app/utils/craftProgress";

test("applies websocket progress without modifying the original snapshot", () => {
  const rows = [{ id: "1", kind: "Active", progress: 2, progress_total: 10, status: "Active", timestamp: null }];
  const next = applyCraftProgress(rows, { craft_id: "1", progress: 10, lock_expiration: "2030-01-01T00:00:00Z" }, 0)!;
  expect(next[0]?.status).toBe("Complete");
  expect(next[0]?.progress).toBe(10);
  expect(rows[0]?.progress).toBe(2);
  expect(applyCraftProgress(rows, { craft_id: "missing", progress: 1, lock_expiration: "2030-01-01T00:00:00Z" })).toBeNull();
});

test("updates passive queue state directly", () => {
  const rows = [{ id: "7", kind: "Passive", status: "Queued", slot: null as number | null, timestamp: "2025-01-01T00:00:00Z" }];
  const next = applyPassiveCraftUpdate(rows, { craft_id: "7", status: "Processing", slot: 2, timestamp: "2025-01-02T00:00:00Z" });
  expect(next?.[0]?.status).toBe("Processing");
  expect(next?.[0]?.slot).toBe(2);
  expect(rows[0]?.status).toBe("Queued");
  expect(applyPassiveCraftUpdate(rows, { craft_id: "8", status: "Complete", slot: null, timestamp: "2025-01-02T00:00:00Z" })).toBeNull();
});

test("uses the new lock time for suspended and active states", () => {
  const rows = [{ id: "1", kind: "Active", progress: 1, progress_total: 10n, status: "Active", timestamp: null }];
  const update = { craft_id: "1", progress: 2, lock_expiration: "2020-01-01T00:00:00Z" };
  expect(applyCraftProgress(rows, update, Date.parse("2021-01-01"))?.[0]?.status).toBe("Suspended");
  expect(applyCraftProgress(rows, update, Date.parse("2019-01-01"))?.[0]?.status).toBe("Active");
});
