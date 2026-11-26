export const MAP_BUILDING_GROUPS = [
  "crafting",
  "refining",
  "trading",
  "housing",
  "storage",
  "other",
] as const;
export type MapBuildingGroup = (typeof MAP_BUILDING_GROUPS)[number];

export const MAP_BUILDING_STYLES = {
  crafting: { label: "Crafting stations", badge: "C", color: "#ffd18b" },
  refining: { label: "Refining stations", badge: "R", color: "#ffad92" },
  trading: { label: "Trading buildings", badge: "T", color: "#9be2ad" },
  housing: { label: "Housing", badge: "H", color: "#c4b0ff" },
  storage: { label: "Storage", badge: "S", color: "#90d9f8" },
  other: { label: "Other buildings", badge: "O", color: "#d0d9e4" },
} satisfies Record<MapBuildingGroup, { label: string; badge: string; color: string }>;

export const MAP_BUILDING_MAX_RESOLUTION = 4;

export interface MapBuilding {
  x: number;
  z: number;
  tier?: number;
  group: MapBuildingGroup;
}

export interface MapBuildingsResponse {
  markers: MapBuilding[];
  truncated: boolean;
}
