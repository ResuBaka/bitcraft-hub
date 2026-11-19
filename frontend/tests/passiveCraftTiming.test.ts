import { expect, test } from "bun:test";
import { passiveCraftTiming } from "../app/utils/passiveCraftTiming";

test("counts down based on processing start and recipe duration", () => {
  const row = { status: "Processing", timestamp: "2026-01-01T00:00:00Z", duration_seconds: 125 };
  const start = Date.parse(row.timestamp);
  expect(passiveCraftTiming([row], start)).toBe("Finishes in 2m 5s");
  expect(passiveCraftTiming([row], start + 6000)).toBe("Finishes in 1m 59s");
  expect(passiveCraftTiming([row], start + 125000)).toBe("Finishing — awaiting server confirmation");
});

test("shows ranges for grouped projects and does not invent queued finish times", () => {
  const start = Date.parse("2026-01-01T00:00:00Z");
  const row = { status: "Processing", timestamp: new Date(start).toISOString(), duration_seconds: 60 };
  expect(passiveCraftTiming([row, { ...row, timestamp: new Date(start + 30000).toISOString() }], start)).toBe("Finishes in 1m 0s – 1m 30s");
  expect(passiveCraftTiming([{ ...row, status: "Queued" }], start)).toBe("Queued — waiting for an available slot");
  expect(passiveCraftTiming([{ ...row, timestamp: null }], start)).toBe("Time remaining unknown");
});

test("completed rows show actual timestamps in local time", () => {
  const first = "2026-01-01T00:00:00Z";
  const second = "2026-01-01T00:01:00Z";
  const row = { status: "Complete", timestamp: first, duration_seconds: 60 };
  expect(passiveCraftTiming([row], 0)).toBe(`Completed: ${new Date(first).toLocaleString()}`);
  expect(passiveCraftTiming([row, { ...row, timestamp: second }], 0)).toBe(`Completed: ${new Date(first).toLocaleString()} – ${new Date(second).toLocaleString()}`);
  expect(passiveCraftTiming([{ ...row, timestamp: null }], 0)).toBe("Completed — time unknown");
});
