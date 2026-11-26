import { SMALL_HEX_WIDTH, SMALL_HEX_ROW_HEIGHT } from "./worldMap";
export { smallHexEdgeSegments as claimBorderSegments } from "./worldMap";

export const CLAIM_BUILDING_KINDS = ["bank", "regionalBank", "market", "waystone"] as const;
export type ClaimBuildingKind = (typeof CLAIM_BUILDING_KINDS)[number];
export const CLAIM_DETAIL_KEYS = ["border", ...CLAIM_BUILDING_KINDS] as const;
export type ClaimDetailKey = (typeof CLAIM_DETAIL_KEYS)[number];
export type ClaimDetailOptions = Record<ClaimDetailKey, boolean>;

export const CLAIM_BUILDING_STYLES = {
  bank: { label: "Claim banks", badge: "B", color: "#e9bc67" },
  regionalBank: { label: "Claim regional banks", badge: "RB", color: "#c4b0ff" },
  market: { label: "Markets", badge: "M", color: "#9be2ad" },
  waystone: { label: "Waystones", badge: "W", color: "#90d9f8" },
} satisfies Record<ClaimBuildingKind, { label: string; badge: string; color: string }>;

export const CLAIM_BORDER_MAX_RESOLUTION = 4;
export const CLAIM_BUILDING_MAX_RESOLUTION = 64;
const MAX_WORLD_CELL = 96_000;
const MAX_SPAN = 2048;
const MAX_AREA = 1_500_000;
const CHUNK_CELLS = 96;

export interface ClaimDetailBounds {
  minX: number;
  minZ: number;
  maxX: number;
  maxZ: number;
}

export type ClaimBorder = [x: number, z: number, mask: number];
export interface ClaimBuilding {
  kind: ClaimBuildingKind;
  x: number;
  z: number;
}
export interface ClaimDetailsResponse {
  borders: ClaimBorder[];
  buildings: ClaimBuilding[];
}

function detailBounds(
  extent: number[],
  buffered: boolean,
  limitBorderArea: boolean,
): ClaimDetailBounds | undefined {
  const [west, south, east, north] = extent;
  if (
    west === undefined ||
    south === undefined ||
    east === undefined ||
    north === undefined ||
    ![west, south, east, north].every(Number.isFinite)
  )
    return;
  const roundDown = (value: number) =>
    buffered ? Math.floor(value / CHUNK_CELLS) * CHUNK_CELLS : Math.floor(value);
  const roundUp = (value: number) =>
    buffered ? Math.ceil(value / CHUNK_CELLS) * CHUNK_CELLS : Math.ceil(value);
  // Allow for staggered rows and hex corners beyond the visible centers.
  const bounds = {
    minX: Math.max(0, roundDown(west / SMALL_HEX_WIDTH - 2)),
    maxX: Math.min(MAX_WORLD_CELL, roundUp(east / SMALL_HEX_WIDTH + 2)),
    minZ: Math.max(0, roundDown(south / SMALL_HEX_ROW_HEIGHT - 2)),
    maxZ: Math.min(MAX_WORLD_CELL, roundUp(north / SMALL_HEX_ROW_HEIGHT + 2)),
  };
  const width = bounds.maxX - bounds.minX + 1;
  const height = bounds.maxZ - bounds.minZ + 1;
  if (
    width <= 0 ||
    height <= 0 ||
    (limitBorderArea && (width > MAX_SPAN || height > MAX_SPAN || width * height > MAX_AREA))
  )
    return;
  return bounds;
}

export function claimDetailBounds(extent: number[], buffered = false) {
  return detailBounds(extent, buffered, true);
}

export function claimBuildingBounds(extent: number[], buffered = false) {
  return detailBounds(extent, buffered, false);
}

export function containsClaimBounds(outer: ClaimDetailBounds, inner: ClaimDetailBounds) {
  return (
    outer.minX <= inner.minX &&
    outer.minZ <= inner.minZ &&
    outer.maxX >= inner.maxX &&
    outer.maxZ >= inner.maxZ
  );
}
