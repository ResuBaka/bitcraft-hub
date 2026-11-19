import { describe, expect, test } from "bun:test";
import { passiveSummaryTiming, summarizePassiveCrafts } from "../app/utils/passiveCraftSummary";

const item = { name: "Brick", icon_asset_name: "brick", tier: 1 };
const craft = {
  id: "1",
  recipe_id: 10,
  recipe_name: "Make brick",
  outputs: [item],
  status: "Complete",
  building_id: "100",
};

describe("passive craft summaries", () => {
  test("groups output across recipes and stations, counting only completed projects as ready", () => {
    const result = summarizePassiveCrafts([
      craft,
      { ...craft, id: "2", recipe_id: 11, building_id: "200", status: "Processing" },
      { ...craft, id: "3", status: "Queued" },
    ]);
    expect(result).toHaveLength(1);
    expect(result[0]).toMatchObject({ name: "Brick", ready: 1, total: 3, buildingId: null });
  });

  test("keeps unidentified recipes separate and avoids counting duplicate outputs twice", () => {
    const result = summarizePassiveCrafts([
      { ...craft, outputs: [item, item] },
      { ...craft, id: "2", outputs: [], recipe_id: 20 },
      { ...craft, id: "3", outputs: [], recipe_id: 21 },
    ]);
    expect(result).toHaveLength(3);
    expect(result[0]).toMatchObject({ total: 1, ready: 1, buildingId: "100" });
  });

  test("counts a project once for each distinct output", () => {
    const result = summarizePassiveCrafts([
      { ...craft, outputs: [item, { name: "Tile", icon_asset_name: "tile", tier: 1 }] },
    ]);
    expect(result.map(({ name, total }) => ({ name, total }))).toEqual([
      { name: "Brick", total: 1 },
      { name: "Tile", total: 1 },
    ]);
  });

  test("shows the earliest estimated processing finish without counting queued projects as done", () => {
    const [summary] = summarizePassiveCrafts([
      { ...craft, status: "Processing", timestamp: "2030-01-01T00:00:00Z", duration_seconds: 120 },
      {
        ...craft,
        id: "2",
        status: "Processing",
        timestamp: "2030-01-01T00:00:00Z",
        duration_seconds: 60,
      },
      { ...craft, id: "3", status: "Queued" },
    ]);
    expect(summary!.ready).toBe(0);
    expect(passiveSummaryTiming(summary!, Date.parse("2030-01-01T00:00:00Z")).label).toBe("1m");
    expect(passiveSummaryTiming(summary!, Date.parse("2030-01-01T00:01:00Z")).label).toBe(
      "Finishing",
    );
    expect(passiveSummaryTiming(summary!, Date.parse("2030-01-01T00:00:00Z")).detail).toContain(
      "Queued",
    );
  });

  test("distinguishes queued, unknown, and complete projects", () => {
    const [queued] = summarizePassiveCrafts([{ ...craft, status: "Queued" }]);
    const [unknown] = summarizePassiveCrafts([{ ...craft, status: "Processing" }]);
    const [done] = summarizePassiveCrafts([craft]);
    expect(passiveSummaryTiming(queued!, 0).label).toBe("Queued");
    expect(passiveSummaryTiming(unknown!, 0).label).toBe("Time unknown");
    expect(passiveSummaryTiming(done!, 0).label).toBe("Done");
  });
});
