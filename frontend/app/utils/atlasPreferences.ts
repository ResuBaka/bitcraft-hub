import { normalizeAtlasResourceColor, type AtlasResourceColors } from "./atlasResources";

export const ATLAS_PREFERENCES_KEY = "bitcraft-atlas-preferences-v1";
export const ATLAS_DISPLAY_SIZE = { min: 75, max: 175, step: 5, default: 100 } as const;

export function atlasDisplaySize(value: unknown) {
  const { min, max, step } = ATLAS_DISPLAY_SIZE;
  if (typeof value !== "number" || !Number.isFinite(value)) return ATLAS_DISPLAY_SIZE.default;
  return Math.min(max, Math.max(min, Math.round(value / step) * step));
}

export interface AtlasPreferences {
  disabledRegions: number[];
  showResourceIcons: boolean;
  showGrid: boolean;
  showRegionNumbers: boolean;
  showRegionNames: boolean;
  resourceColors: AtlasResourceColors;
  iconSize: number;
  infoSize: number;
}

export function setAtlasRegionsEnabled(
  disabledRegions: readonly number[],
  regionIds: readonly number[],
  enabled: boolean,
) {
  const disabled = new Set(disabledRegions);
  for (const id of regionIds) {
    if (enabled) disabled.delete(id);
    else disabled.add(id);
  }
  return [...disabled];
}

export function atlasRegionScope(regionIds: readonly number[], disabled: ReadonlySet<number>) {
  const enabled = regionIds.filter((id) => !disabled.has(id));
  return enabled.length === regionIds.length
    ? undefined
    : enabled.sort((a, b) => a - b).join(",") || "none";
}

export function readAtlasPreferences(raw: string | null): AtlasPreferences {
  const defaults = {
    disabledRegions: [],
    showResourceIcons: true,
    showGrid: false,
    showRegionNumbers: false,
    showRegionNames: false,
    resourceColors: {},
    iconSize: ATLAS_DISPLAY_SIZE.default,
    infoSize: ATLAS_DISPLAY_SIZE.default,
  };
  if (!raw) return defaults;
  try {
    const saved:
      | (Partial<AtlasPreferences> & { hiddenRegions?: number[]; displaySize?: number })
      | null = JSON.parse(raw);
    const disabledRegions = saved?.disabledRegions ?? saved?.hiddenRegions;
    const resourceColors: Record<number, string> = {};
    for (const [key, value] of Object.entries(saved?.resourceColors ?? {})) {
      if (!/^-?\d{1,10}$/.test(key)) continue;
      const id = Number(key);
      const color = normalizeAtlasResourceColor(value);
      if (color && id >= -2_147_483_648 && id <= 2_147_483_647) resourceColors[id] = color;
    }
    return {
      disabledRegions: Array.isArray(disabledRegions)
        ? [...new Set(disabledRegions.filter((id) => Number.isInteger(id) && id > 0 && id <= 255))]
        : [],
      showResourceIcons: saved?.showResourceIcons !== false,
      showGrid: saved?.showGrid === true,
      showRegionNumbers: saved?.showRegionNumbers === true,
      showRegionNames: saved?.showRegionNames === true,
      resourceColors,
      iconSize: atlasDisplaySize(saved?.iconSize ?? saved?.displaySize),
      infoSize: atlasDisplaySize(saved?.infoSize ?? saved?.displaySize),
    };
  } catch {
    return defaults;
  }
}
