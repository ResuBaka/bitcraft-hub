import { CLAIM_DETAIL_KEYS, type ClaimDetailOptions } from "./claimMapDetails";
import { MAP_BUILDING_GROUPS, type MapBuildingGroup } from "./mapBuildings";
import { atlasResourceIds } from "./atlasResources";

export const MAP_LAYERS = [
  "terrain",
  "claims",
  "buildings",
  "players",
  "roads",
  "resources",
] as const;
type MapLayer = (typeof MAP_LAYERS)[number];

const DEFAULT_LAYERS = {
  terrain: true,
  claims: false,
  buildings: false,
  players: false,
  roads: false,
  resources: false,
};

export interface MapCamera {
  space: "world" | "grid";
  x: number;
  z: number;
  zoom: number;
}

export interface MapUrlState {
  layers: Record<MapLayer, boolean>;
  claimDetails: ClaimDetailOptions;
  claimTiers?: number[];
  buildingGroups: MapBuildingGroup[];
  resourceIds: number[];
  camera?: MapCamera;
  region?: number;
  claim?: string;
}

function readNumber(value: unknown): number | undefined {
  if (typeof value !== "string" || !/^-?\d+(?:\.\d+)?$/.test(value)) return;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}

export function readMapUrl(query: Readonly<Record<string, unknown>>): MapUrlState {
  const layers = { ...DEFAULT_LAYERS };
  if (typeof query.layers === "string") {
    const enabled = new Set(query.layers.split(","));
    // Older links can explicitly show only region decorations; preserve
    // their other layer choices without importing any local preferences.
    if (
      query.layers === "none" ||
      MAP_LAYERS.some((name) => enabled.has(name)) ||
      ["grid", "numbers", "names"].some((name) => enabled.has(name))
    ) {
      for (const name of MAP_LAYERS) layers[name] = enabled.has(name);
    }
  }

  const x = readNumber(query.x);
  const z = readNumber(query.z);
  const zoom = readNumber(query.zoom);
  const space = query.space === "grid" ? "grid" : "world";
  const camera =
    (query.space === undefined || query.space === "world" || query.space === "grid") &&
    x !== undefined &&
    Math.abs(x) <= 1_000_000 &&
    z !== undefined &&
    Math.abs(z) <= 1_000_000 &&
    zoom !== undefined &&
    zoom >= 0 &&
    zoom <= 24
      ? { space, x, z, zoom }
      : undefined;

  const region = readNumber(query.region);
  const detailValues =
    typeof query.details === "string" ? new Set(query.details.split(",")) : undefined;
  const customDetails =
    detailValues &&
    (detailValues.has("none") || CLAIM_DETAIL_KEYS.some((key) => detailValues.has(key)));
  const claimDetails = Object.fromEntries(
    CLAIM_DETAIL_KEYS.map((key) => [key, !customDetails || detailValues?.has(key) === true]),
  ) as ClaimDetailOptions;
  const tiers =
    typeof query.tiers === "string"
      ? [
          ...new Set(
            query.tiers
              .split(",")
              .map(readNumber)
              .filter(
                (tier): tier is number =>
                  tier !== undefined && Number.isInteger(tier) && tier >= 0 && tier <= 99,
              ),
          ),
        ].sort((a, b) => a - b)
      : undefined;
  const groupValues =
    typeof query.buildingGroups === "string" ? new Set(query.buildingGroups.split(",")) : undefined;
  const buildingGroups = MAP_BUILDING_GROUPS.filter((group) => groupValues?.has(group));
  return {
    layers,
    claimDetails,
    claimTiers: query.tiers === "none" || tiers?.length ? (tiers ?? []) : undefined,
    buildingGroups:
      buildingGroups.length || query.buildingGroups === "none"
        ? buildingGroups
        : [...MAP_BUILDING_GROUPS],
    resourceIds: atlasResourceIds(
      typeof query.resources === "string"
        ? query.resources
            .split(",", 64)
            .map((entry) => (/^-?\d{1,10}$/.test(entry) ? Number(entry) : NaN))
        : [],
    ),
    camera,
    region:
      region !== undefined && Number.isInteger(region) && region >= 1 && region <= 255
        ? region
        : undefined,
    claim:
      layers.claims && typeof query.claim === "string" && /^[1-9]\d{0,19}$/.test(query.claim)
        ? query.claim
        : undefined,
  };
}

export function writeMapUrl(state: MapUrlState) {
  const { camera, layers, region, claim, claimDetails, claimTiers, buildingGroups, resourceIds } =
    state;
  const number = (value: number) => String(Number(value.toFixed(4)));
  const details = CLAIM_DETAIL_KEYS.filter((key) => claimDetails[key]);
  const groups = MAP_BUILDING_GROUPS.filter((group) => buildingGroups.includes(group));
  return {
    layers: MAP_LAYERS.filter((name) => layers[name]).join(",") || "none",
    details: details.length === CLAIM_DETAIL_KEYS.length ? undefined : details.join(",") || "none",
    tiers: claimTiers === undefined ? undefined : claimTiers.join(",") || "none",
    buildingGroups:
      groups.length === MAP_BUILDING_GROUPS.length ? undefined : groups.join(",") || "none",
    resources: atlasResourceIds(resourceIds).join(",") || undefined,
    x: camera && number(camera.x),
    z: camera && number(camera.z),
    zoom: camera && number(camera.zoom),
    space: camera?.space === "grid" ? "grid" : undefined,
    region: region === undefined ? undefined : String(region),
    claim: layers.claims ? claim : undefined,
  };
}
