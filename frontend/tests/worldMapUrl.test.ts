import { describe, expect, test } from "bun:test";
import { readMapUrl, readSavedMapView, splitMapQuery, writeMapUrl } from "../app/utils/worldMapUrl";
import { MAP_BUILDING_GROUPS } from "../app/utils/mapBuildings";

describe("world map links", () => {
  test("stores the shared view locally and only strips map parameters from the URL", () => {
    const query = {
      x: "81234",
      z: "12345",
      zoom: "6.5",
      layers: "terrain,resources",
      resources: "2",
      source: "chat",
    };
    expect(splitMapQuery(query)).toEqual({ shared: true, other: { source: "chat" } });
    expect(splitMapQuery({ source: "chat" }).shared).toBe(false);
    const state = readMapUrl(query);
    expect(readSavedMapView(JSON.stringify(writeMapUrl(state)))).toEqual(state);
    expect(readSavedMapView("invalid JSON")).toEqual(readMapUrl({}));
  });
  test("round-trips position, fractional zoom, visible layers, region and the exact claim ID", () => {
    const state = readMapUrl({
      x: "81234.125",
      z: "-19.25",
      zoom: "6.5",
      layers: "terrain,names,claims",
      region: "13",
      claim: "504403158266578089",
    });
    expect(state).toEqual({
      camera: { x: 81234.125, z: -19.25, zoom: 6.5, space: "world" },
      layers: {
        terrain: true,
        claims: true,
        buildings: false,
        players: false,
        roads: false,
        resources: false,
      },
      claimDetails: { border: true, bank: true, regionalBank: true, market: true, waystone: true },
      buildingGroups: [...MAP_BUILDING_GROUPS],
      resourceIds: [],
      region: 13,
      claim: "504403158266578089",
    });
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
    expect(writeMapUrl(state).layers).toBe("terrain,claims");
  });

  test("old region decoration tokens do not override browser preferences or other layer choices", () => {
    const state = readMapUrl({ layers: "grid,numbers,names", region: "11" });
    expect(Object.values(state.layers).every((enabled) => !enabled)).toBe(true);
    expect(writeMapUrl(state)).toMatchObject({ layers: "none", region: "11" });
  });

  test("preserves claim sublayers and tier choices independently of the master switch", () => {
    const state = readMapUrl({ layers: "terrain", details: "border,regionalBank", tiers: "5,2,5" });
    expect(state.layers.claims).toBe(false);
    expect(state.claimDetails).toEqual({
      border: true,
      bank: false,
      regionalBank: true,
      market: false,
      waystone: false,
    });
    expect(state.claimTiers).toEqual([2, 5]);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
    expect(readMapUrl({ tiers: "none", details: "none" }).claimTiers).toEqual([]);
    expect(
      Object.values(readMapUrl({ details: "none" }).claimDetails).every((enabled) => !enabled),
    ).toBe(true);
  });

  test("distinguishes schematic coordinates from the world and preserves all layers off", () => {
    const state = readMapUrl({ x: "2.052", z: "0", zoom: "3", space: "grid", layers: "none" });
    expect(state.camera?.space).toBe("grid");
    expect(Object.values(state.layers).every((enabled) => !enabled)).toBe(true);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
  });

  test("round-trips building group filters independently of the Buildings and Claims layers", () => {
    const state = readMapUrl({
      layers: "buildings",
      buildingGroups: "housing,crafting,housing,unknown",
    });
    expect(state.layers.buildings).toBe(true);
    expect(state.layers.claims).toBe(false);
    expect(state.buildingGroups).toEqual(["crafting", "housing"]);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
    const hidden = readMapUrl({ layers: "none", buildingGroups: "none" });
    expect(hidden.buildingGroups).toEqual([]);
    expect(readMapUrl(writeMapUrl(hidden))).toEqual(hidden);
  });

  test("ignores unusable values and does not open claims from hidden layers", () => {
    const defaults = readMapUrl({});
    expect(
      readMapUrl({ x: ["5"], z: null, zoom: "5000", layers: "typo", region: "1.5", claim: "42" }),
    ).toEqual(defaults);
    expect(readMapUrl({ x: "1", z: "2", zoom: "3", space: "typo" }).camera).toBeUndefined();
    expect(readMapUrl({ layers: "claims", claim: "not-an-id" }).claim).toBeUndefined();
  });

  test("online players are opt-in and persist in shared links", () => {
    expect(readMapUrl({}).layers.players).toBe(false);
    const state = readMapUrl({ layers: "terrain,players" });
    expect(state.layers.players).toBe(true);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
  });

  test("paving is opt-in and persists in shared links independently of terrain", () => {
    expect(readMapUrl({}).layers.roads).toBe(false);
    const state = readMapUrl({ layers: "roads" });
    expect(state.layers.roads).toBe(true);
    expect(state.layers.terrain).toBe(false);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
  });

  test("resource IDs are bounded and saved even with the Resources layer off", () => {
    const state = readMapUrl({
      layers: "terrain",
      resources: "12,-1,12,1e3,2147483648,8,7,6,5,4,3,2,1",
    });
    expect(state.layers.resources).toBe(false);
    expect(state.resourceIds).toEqual([-1, 3, 4, 5, 6, 7, 8, 12]);
    expect(readMapUrl(writeMapUrl(state))).toEqual(state);
    expect(readMapUrl({ layers: "resources" }).layers.resources).toBe(true);
  });
});
