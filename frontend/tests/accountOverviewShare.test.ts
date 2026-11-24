import { describe, expect, test } from "bun:test";
import {
  decodeTrackingSetup,
  encodeTrackingSetup,
  type SharedDashboard,
} from "../app/utils/accountOverviewShare";

const dashboard: SharedDashboard = {
  id: "360287970367149239",
  name: "玩家 🚀",
  position: { x: 4, y: 0, w: 8, h: 5 },
  layout: {
    format: "bitcraft-hub-player-layout",
    version: 1,
    blocks: [
      { id: "stats", x: 0, y: 0, w: 6, h: 3 },
      { id: "active-crafts", x: 6, y: 0, w: 6, h: 3 },
      { id: "passive-crafts", x: 0, y: 3, w: 12, h: 3 },
      { id: "activity", x: 0, y: 6, w: 6, h: 3 },
      { id: "placeables", x: 6, y: 6, w: 6, h: 3 },
      { id: "farming", x: 0, y: 9, w: 6, h: 3 },
    ],
  },
  activeClaims: ["claim:360287970367149240", "unclaimed"],
  passiveClaims: ["unknown"],
};

describe("account overview setup links", () => {
  test("round trips a full setup as a URL fragment without losing big IDs or names", () => {
    const players: SharedDashboard[] = [
      dashboard,
      {
        ...dashboard,
        id: "360287970367149241",
        position: { x: 0, y: 0, w: 4, h: 3 },
        layout: { ...dashboard.layout, blocks: [] },
        activeClaims: [],
        passiveClaims: [],
      },
    ];
    const hash = encodeTrackingSetup(players)!;
    expect(hash).toMatch(/^#setup=[A-Za-z0-9_-]+$/);
    const url = new URL(`https://example.test/tools/account-overview${hash}`);
    expect(decodeTrackingSetup(url.hash)).toEqual(players);
  });

  test("rejects malformed, oversized, and unsupported links", () => {
    expect(decodeTrackingSetup("#elsewhere")).toBeNull();
    expect(decodeTrackingSetup("#setup=!!")).toBeNull();
    expect(decodeTrackingSetup("#setup=WzIsW11d")).toBeNull(); // [2,[]]
    expect(decodeTrackingSetup(`#setup=${"A".repeat(16_001)}`)).toBeNull();
  });

  test("rejects duplicate players, invalid IDs, positions, blocks, and filters", () => {
    expect(encodeTrackingSetup([dashboard, dashboard])).toBeNull();
    expect(encodeTrackingSetup([{ ...dashboard, id: "18446744073709551616" }])).toBeNull();
    expect(
      encodeTrackingSetup([{ ...dashboard, position: { x: 10, y: 0, w: 4, h: 3 } }]),
    ).toBeNull();
    expect(
      encodeTrackingSetup([
        {
          ...dashboard,
          layout: {
            ...dashboard.layout,
            blocks: [dashboard.layout.blocks[0]!, dashboard.layout.blocks[0]!],
          },
        },
      ]),
    ).toBeNull();
    expect(encodeTrackingSetup([{ ...dashboard, activeClaims: ["claim:invalid"] }])).toBeNull();
  });
});
