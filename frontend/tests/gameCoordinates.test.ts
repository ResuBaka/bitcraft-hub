import { expect, test } from "bun:test";
import { mobilePlayerCoordinate } from "../app/utils/atlasPlayers";
import {
  gameCoordinates,
  mapChatCoordinates,
  mapGameCoordinates,
} from "../app/utils/gameCoordinates";

test("the atlas readout matches the player's compass coordinates", () => {
  expect(gameCoordinates(28_473_000, 18_888_000)).toEqual({ north: 6_296, east: 9_491 });
  for (const [x, z] of [
    [0, 0],
    [2_999, 3_001],
    [3_000, 2_999],
    [28_473_000, 18_888_000],
    [-3_001, -2_999],
    [-3_000, -3_001],
  ] as const) {
    expect(mapGameCoordinates(...mobilePlayerCoordinate(x, z))).toEqual(gameCoordinates(x, z));
  }
});

test("copies in-game chat coordinates in north,east order with parentheses", () => {
  expect(mapChatCoordinates(...mobilePlayerCoordinate(25_227_000, 18_741_000))).toBe(
    "(coord=6247,8409)",
  );
});
