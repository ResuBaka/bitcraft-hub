import { expect, test } from "bun:test";
import {
  atlasRegionScope,
  readAtlasPreferences,
  setAtlasRegionsEnabled,
} from "../app/utils/atlasPreferences";
import { CURRENT_WORLD_LAYOUT, isAtlasEventRegion } from "../app/utils/worldMap";
import { readMapUrl } from "../app/utils/worldMapUrl";

test("local preferences default to visible regions and icons", () => {
  const defaults = {
    disabledRegions: [],
    showResourceIcons: true,
    showPlayerNames: false,
    terrainSaturation: 100,
    pavingSaturation: 100,
    showGrid: false,
    showRegionNumbers: false,
    showRegionNames: false,
    resourceColors: {},
    iconSize: 100,
    infoSize: 100,
  };
  expect(readAtlasPreferences(null)).toEqual(defaults);
  expect(readAtlasPreferences("invalid JSON")).toEqual(readAtlasPreferences(null));
  expect(
    readAtlasPreferences('{"hiddenRegions":[7,7,25,"8",-1,1.5],"showResourceIcons":false}'),
  ).toEqual({
    ...defaults,
    disabledRegions: [7, 25],
    showResourceIcons: false,
  });
});

test("terrain and paving saturations are independent and stay out of shared links", () => {
  expect(readAtlasPreferences('{"vibrantPaving":true}')).toMatchObject({
    pavingSaturation: 155,
    terrainSaturation: 100,
  });
  expect(readAtlasPreferences('{"vibrantTerrain":true}').terrainSaturation).toBe(155);
  expect(readAtlasPreferences('{"vibrantTerrain":"true"}').terrainSaturation).toBe(100);
  expect(readAtlasPreferences('{"terrainSaturation":342,"pavingSaturation":450}')).toMatchObject({
    terrainSaturation: 340,
    pavingSaturation: 400,
  });
  expect(
    readAtlasPreferences('{"vibrantTerrain":true,"terrainSaturation":100}').terrainSaturation,
  ).toBe(100);
  const state = readMapUrl({ terrainSaturation: "150", pavingSaturation: "200" });
  expect(state).not.toHaveProperty("terrainSaturation");
  expect(state).not.toHaveProperty("pavingSaturation");
});

test("map player names are opt-in and kept out of shared state", () => {
  expect(readAtlasPreferences('{"showPlayerNames":true}').showPlayerNames).toBe(true);
  expect(readAtlasPreferences('{"showPlayerNames":"true"}').showPlayerNames).toBe(false);
  expect(readMapUrl({ showPlayerNames: "true" })).not.toHaveProperty("showPlayerNames");
});

test("icon and info sizes are independent browser-only preferences", () => {
  expect(readAtlasPreferences('{"iconSize":147,"infoSize":999}')).toMatchObject({
    iconSize: 145,
    infoSize: 175,
  });
  expect(readAtlasPreferences('{"iconSize":0,"infoSize":"150"}')).toMatchObject({
    iconSize: 75,
    infoSize: 100,
  });
  const shared = readMapUrl({ iconSize: "150", infoSize: "125" });
  expect(shared).not.toHaveProperty("iconSize");
  expect(shared).not.toHaveProperty("infoSize");
});

test("the old combined size carries over to both new sliders", () => {
  expect(readAtlasPreferences('{"displaySize":150}')).toMatchObject({
    iconSize: 150,
    infoSize: 150,
  });
  expect(readAtlasPreferences('{"displaySize":150,"iconSize":100}')).toMatchObject({
    iconSize: 100,
    infoSize: 150,
  });
});

test("saved resource colors accept only hex colors and valid resource IDs", () => {
  expect(
    readAtlasPreferences(
      '{"resourceColors":{"2":"#ABC","-7":"015a4C","8":"orange","invalid":"#ffffff","2147483648":"#ffffff"}}',
    ).resourceColors,
  ).toEqual({ 2: "#aabbcc", [-7]: "#015a4c" });
});

test("region decoration preferences are opt-in and persist as browser settings", () => {
  expect(
    readAtlasPreferences('{"showGrid":true,"showRegionNumbers":true,"showRegionNames":true}'),
  ).toMatchObject({ showGrid: true, showRegionNumbers: true, showRegionNames: true });
});

test("event region controls leave the nine main regions' saved visibility alone", () => {
  const { regions } = CURRENT_WORLD_LAYOUT;
  const main = regions.filter(({ id }) => !isAtlasEventRegion(id)).map(({ id }) => id);
  const events = regions.filter(({ id }) => isAtlasEventRegion(id)).map(({ id }) => id);
  expect(main).toEqual([7, 8, 9, 12, 13, 14, 17, 18, 19]);
  expect(events).toHaveLength(16);
  const hidden = setAtlasRegionsEnabled([7, 14], events, false);
  expect(hidden).toEqual([7, 14, ...events]);
  expect(setAtlasRegionsEnabled(hidden, events, true)).toEqual([7, 14]);
  const hiddenMain = setAtlasRegionsEnabled([3, 11], main, false);
  expect(hiddenMain).toEqual([3, 11, ...main]);
  expect(setAtlasRegionsEnabled(hiddenMain, main, true)).toEqual([3, 11]);
});

test("shared locations don't depend on local visibility preferences", () => {
  const shared = readMapUrl({ region: "7", x: "110", z: "22", zoom: "6", hiddenRegions: "7" });
  expect(shared.region).toBe(7);
  expect(shared.camera).toMatchObject({ x: 110, z: 22, zoom: 6 });
  expect(shared).not.toHaveProperty("hiddenRegions");
  expect(shared).not.toHaveProperty("disabledRegions");
});

test("overlay requests scope to enabled region IDs and distinguish none from all", () => {
  expect(atlasRegionScope([9, 7, 8], new Set())).toBeUndefined();
  expect(atlasRegionScope([9, 7, 8], new Set([8]))).toBe("7,9");
  expect(atlasRegionScope([9, 7, 8], new Set([7, 8, 9]))).toBe("none");
});
