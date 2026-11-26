import { SMALL_HEX_ROW_HEIGHT, SMALL_HEX_WIDTH } from "./worldMap";

type Player = [id: string, x: number, z: number];
export type AtlasPlayerUpdate =
  | { type: "snapshot"; players: Player[] }
  | { type: "delta"; upsert: Player[]; remove: string[] };

const validId = (id: unknown): id is string => typeof id === "string" && /^[0-9]{1,20}$/.test(id);
const validPlayer = (value: unknown): value is Player =>
  Array.isArray(value) &&
  value.length === 3 &&
  validId(value[0]) &&
  Number.isSafeInteger(value[1]) &&
  Number.isSafeInteger(value[2]);

export function decodeAtlasPlayerUpdate(message: string): AtlasPlayerUpdate | undefined {
  try {
    const data = JSON.parse(message);
    if (data?.type === "snapshot" && Array.isArray(data.players) && data.players.every(validPlayer))
      return data;
    if (
      data?.type === "delta" &&
      Array.isArray(data.upsert) &&
      data.upsert.every(validPlayer) &&
      Array.isArray(data.remove) &&
      data.remove.every(validId)
    )
      return data;
  } catch {
    // Ignore malformed frames.
  }
}

export function mobilePlayerCoordinate(x: number, z: number): [number, number] {
  // Mirrors BitCraft's FloatHexTile: the offset-to-axial conversion uses
  // integer division before scaling by 1/1000 of a small hex.
  return [
    ((x - Math.trunc(z / 2) + z / 2) / 1_000) * SMALL_HEX_WIDTH,
    (z / 1_000) * SMALL_HEX_ROW_HEIGHT,
  ];
}
