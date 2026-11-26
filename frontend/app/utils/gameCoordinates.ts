import { SMALL_HEX_ROW_HEIGHT, SMALL_HEX_WIDTH } from "./worldMap";

export function gameCoordinates(locationX: number, locationZ: number) {
  // The game's compass labels count one unit per three small hexes.
  return { north: Math.floor(locationZ / 3_000), east: Math.floor(locationX / 3_000) };
}

export function mapGameCoordinates(x: number, z: number) {
  // Undo mobilePlayerCoordinate before applying the player page's display
  // formula. Round in the raw units so exact cell boundaries survive FP error.
  const locationZ = Math.round((z / SMALL_HEX_ROW_HEIGHT) * 1_000);
  const rowShift = locationZ / 2 - Math.trunc(locationZ / 2);
  const locationX = Math.round((x / SMALL_HEX_WIDTH) * 1_000 - rowShift);
  return gameCoordinates(locationX, locationZ);
}
