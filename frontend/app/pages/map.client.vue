<script setup lang="ts">
import "ol/ol.css";
import Feature from "ol/Feature.js";
import OlMap from "ol/Map.js";
import Overlay from "ol/Overlay.js";
import View from "ol/View.js";
import { defaults as controls } from "ol/control/defaults.js";
import { buffer, containsCoordinate, createEmpty, extend, getCenter, getWidth } from "ol/extent.js";
import Point from "ol/geom/Point.js";
import { fromExtent } from "ol/geom/Polygon.js";
import { defaults as interactions } from "ol/interaction/defaults.js";
import TileLayer from "ol/layer/Tile.js";
import VectorLayer from "ol/layer/Vector.js";
import type { Pixel } from "ol/pixel.js";
import Projection from "ol/proj/Projection.js";
import VectorSource from "ol/source/Vector.js";
import XYZ from "ol/source/XYZ.js";
import { Circle as CircleStyle, Fill, Stroke, Style, Text } from "ol/style.js";
import TileGrid from "ol/tilegrid/TileGrid.js";
import AtlasLayerGroup from "~/components/Atlas/LayerGroup.vue";
import AtlasMonitorPanel from "~/components/Atlas/MonitorPanel.vue";
import AtlasResourceColorPicker from "~/components/Atlas/ResourceColorPicker.vue";
import AtlasResourceKey from "~/components/Atlas/ResourceKey.vue";
import { withAtlasBrowserCache, type AtlasFetch } from "~/utils/atlasBrowserCache";
import { createAtlasMarkerScale, type AtlasMarkerScale } from "~/utils/atlasMarkerScale";
import { MAX_FAVORITES } from "~/utils/atlasMonitor";
import { createClaimDetailsOverlay, type ClaimDetailsStatus } from "~/utils/claimDetailsOverlay";
import { createAtlasPlayersOverlay, type AtlasPlayersStatus } from "~/utils/atlasPlayersOverlay";
import {
  ATLAS_DISPLAY_SIZE,
  ATLAS_PREFERENCES_KEY,
  ATLAS_SATURATION,
  atlasRegionScope,
  readAtlasPreferences,
  setAtlasRegionsEnabled,
} from "~/utils/atlasPreferences";
import { createAtlasRoadsOverlay, type AtlasRoadsStatus } from "~/utils/atlasRoadsOverlay";
import {
  atlasResourceColor,
  atlasResourceIconUrl,
  atlasResourceIds,
  matchAtlasResource,
  MAX_RESOURCE_FILTERS,
  type AtlasResourceColors,
  type AtlasResourceType,
} from "~/utils/atlasResources";
import { createAtlasResourcesOverlay, type ResourcesStatus } from "~/utils/atlasResourcesOverlay";
import { ATLAS_MAX_TILES_LOADING, enableAtlasZoomTileLoading } from "~/utils/atlasTileLoading";
import { vibrantAtlasColor } from "~/utils/atlasVibrantColors";
import { mapChatCoordinates, mapGameCoordinates } from "~/utils/gameCoordinates";
import { MAP_BUILDING_GROUPS, MAP_BUILDING_STYLES } from "~/utils/mapBuildings";
import { createMapBuildingsOverlay, type MapBuildingsStatus } from "~/utils/mapBuildingsOverlay";
import { createTerrainHexOverlay } from "~/utils/terrainHexOverlay";
import { TERRAIN_HEX_MAX_SOURCE_RESOLUTION, TERRAIN_MIN_RESOLUTION } from "~/utils/terrainHexes";
import {
  CLAIM_BUILDING_KINDS,
  CLAIM_BUILDING_STYLES,
  CLAIM_DETAIL_KEYS,
} from "~/utils/claimMapDetails";
import {
  atlasTileKey,
  claimBadgeSize,
  compareClaimMarkerPriority,
  CURRENT_WORLD_LAYOUT,
  isAtlasEventRegion,
  regionCoverage,
  regionExtent,
  schematicRegionExtent,
  smallHexCenter,
  type WorldMapAtlas,
  type WorldMapClaim,
  type WorldMapLayout,
  type WorldMapRegion,
} from "~/utils/worldMap";
import {
  ATLAS_VIEW_KEY,
  readMapUrl,
  readSavedMapView,
  splitMapQuery,
  writeMapUrl,
  type MapCamera,
  type MapUrlState,
} from "~/utils/worldMapUrl";

definePageMeta({ layout: "map" });

useSeoMeta({
  title: "World Atlas · BitCraft Hub",
  description: "Explore the regions of the BitCraft overworld.",
});

const target = useTemplateRef<HTMLElement>("target");
const layersButton = useTemplateRef<HTMLElement>("layersButton");
const router = useRouter();
const mapRoute = router.currentRoute.value.matched.at(-1);
const config = useRuntimeConfig();
const currentBase = config.public.api.base
  ? `${config.public.api.base.replace(/\/+$/, "")}/world-map/`
  : undefined;
const monitor = useAtlasMonitor(currentBase);
const {
  preferences: monitorPreferences,
  rosters: monitorRosters,
  rosterLoading: monitorRosterLoading,
  rosterError: monitorRosterError,
  targets: monitorTargets,
} = monitor;
const visibleTrackedPlayers = shallowRef<ReadonlySet<string>>(new Set());

function cachedMapQuery<Query extends object, Data>(endpoint: string): AtlasFetch<Query, Data> {
  return withAtlasBrowserCache(endpoint, window.location.href, (query, signal) =>
    $fetch<Data>(endpoint, { query, signal, cache: "no-cache", retry: 0, timeout: 10_000 }),
  );
}
const atlas = shallowRef<WorldMapLayout>();
const openLayerOptions = ref<string>();
const disabledRegions = ref<number[]>([]);
const disabledRegionIds = computed(() => new Set(disabledRegions.value));
const showResourceIcons = ref(true);
const showPlayerNames = ref(false);
const showGrid = ref(false);
const showRegionNumbers = ref(false);
const showRegionNames = ref(false);
const resourceColors = shallowRef<AtlasResourceColors>({});
const displaySizes = reactive<
  Record<"iconSize" | "infoSize" | "terrainSaturation" | "pavingSaturation", number>
>({
  iconSize: ATLAS_DISPLAY_SIZE.default,
  infoSize: ATLAS_DISPLAY_SIZE.default,
  terrainSaturation: ATLAS_SATURATION.default,
  pavingSaturation: ATLAS_SATURATION.default,
});
const displaySizeControls = [
  { key: "terrainSaturation", label: "Terrain saturation", range: ATLAS_SATURATION },
  { key: "pavingSaturation", label: "Paving saturation", range: ATLAS_SATURATION },
  { key: "iconSize", label: "Icon size", range: ATLAS_DISPLAY_SIZE },
  { key: "infoSize", label: "Info size", range: ATLAS_DISPLAY_SIZE },
] as const;
const terrainSaturation = computed(() => displaySizes.terrainSaturation / 100);
const pavingSaturation = computed(() => displaySizes.pavingSaturation / 100);
const iconFactor = computed(() => displaySizes.iconSize / 100);
const infoFactor = computed(() => displaySizes.infoSize / 100);
let markerScale: AtlasMarkerScale | undefined;
let labelScale: AtlasMarkerScale | undefined;
let preferencesLoaded = false;

function resourceColor(id: number) {
  return atlasResourceColor(id, resourceColors.value);
}

function setResourceColor(id: number, color: string | undefined) {
  const next = { ...resourceColors.value };
  if (!color || color === atlasResourceColor(id)) delete next[id];
  else next[id] = color;
  resourceColors.value = next;
}

function setRegionEnabled(id: number, enabled: boolean) {
  disabledRegions.value = setAtlasRegionsEnabled(disabledRegions.value, [id], enabled);
}

function setAllRegionsEnabled(enabled: boolean) {
  disabledRegions.value = enabled ? [] : listedRegions.value.map(({ id }) => id);
}

function setRegionGroupEnabled(regions: readonly WorldMapRegion[], enabled: boolean) {
  disabledRegions.value = setAtlasRegionsEnabled(
    disabledRegions.value,
    regions.map(({ id }) => id),
    enabled,
  );
}

function setLayerOptionsOpen(layer: string, open: boolean) {
  if (open) openLayerOptions.value = layer;
  else if (openLayerOptions.value === layer) openLayerOptions.value = undefined;
}

function onLayerMenuToggle(event: Event) {
  if (!(event.currentTarget as HTMLDetailsElement).open) openLayerOptions.value = undefined;
}
const oceanBackdropStyle = computed(() => {
  const color = atlas.value?.backgroundColor;
  const base = color && /^#[a-f\d]{6}$/i.test(color) ? color : "#193a4a";
  return {
    "--atlas-ocean-color":
      terrainSaturation.value !== 1 && showTerrain.value && currentHasTerrain.value
        ? vibrantAtlasColor(base, terrainSaturation.value)
        : base,
  };
});
const listedRegions = computed(() =>
  [...(atlas.value?.regions ?? [])].sort((a, b) => b.z - a.z || a.x - b.x),
);
const regionScope = computed(() =>
  atlasRegionScope(
    listedRegions.value.map(({ id }) => id),
    disabledRegionIds.value,
  ),
);
const settingsRegionGroups = computed(() =>
  [
    {
      label: "Main regions",
      regions: listedRegions.value.filter(({ id }) => !isAtlasEventRegion(id)),
    },
    {
      label: "Event regions",
      regions: listedRegions.value.filter(({ id }) => isAtlasEventRegion(id)),
    },
  ].filter(({ regions }) => regions.length),
);
const selected = shallowRef<WorldMapRegion>();
const currentHasTerrain = ref(false);
function readCurrentMapState() {
  const { query } = router.currentRoute.value;
  if (splitMapQuery(query).shared) return readMapUrl(query);
  try {
    return readSavedMapView(localStorage.getItem(ATLAS_VIEW_KEY));
  } catch {
    return readMapUrl({});
  }
}
const initialMapState = readCurrentMapState();
const layers = reactive(initialMapState.layers);
const claimDetails = reactive(initialMapState.claimDetails);
const buildingGroups = ref(initialMapState.buildingGroups);
const buildingsStatus = ref<MapBuildingsStatus>("idle");
const atlasPlayersStatus = ref<AtlasPlayersStatus>("off");
const atlasRoadsStatus = ref<AtlasRoadsStatus>("off");
const resourcesStatus = ref<ResourcesStatus>("off");
const resourceIds = ref(initialMapState.resourceIds);
const favoriteResourceIds = computed(
  () =>
    new Set(
      monitorPreferences.value.favorites
        .filter(({ kind }) => kind === "resource")
        .map(({ id }) => Number(id)),
    ),
);
const favoritesFull = computed(() => monitorPreferences.value.favorites.length >= MAX_FAVORITES);
const resourceSearch = ref("");
const resourceTypes = shallowRef<AtlasResourceType[]>([]);
const resourceIcons = computed(() => {
  const icons = new Map<number, string>();
  if (!showResourceIcons.value) return icons;
  for (const type of resourceTypes.value) {
    const url = atlasResourceIconUrl(type.icon_asset_name, config.public.iconDomain);
    if (url) icons.set(type.id, url);
  }
  return icons;
});
const resourceCatalogStatus = ref<"idle" | "loading" | "ready" | "error">("idle");
const selectedResourceTypes = computed(() =>
  resourceIds.value.map(
    (id) =>
      resourceTypes.value.find((type) => type.id === id) ?? {
        id,
        name: `Resource #${id}`,
        tag: "",
        tier: 0,
      },
  ),
);
const resourceChoices = computed(() => {
  const selected = new Set(resourceIds.value);
  const available = resourceTypes.value;
  const matching = available.filter(
    (type) => !selected.has(type.id) && matchAtlasResource(type, resourceSearch.value),
  );
  return {
    options: [...selectedResourceTypes.value, ...matching.slice(0, 30)],
    more: Math.max(0, matching.length - 30),
  };
});
const onlinePlayerCount = ref(0);
const claimTiers = ref(initialMapState.claimTiers);
const observedTiers = shallowRef<number[]>([]);
const selectableClaimTiers = computed(() =>
  [
    ...new Set([2, 3, 4, 5, 6, 7, 8, 9, 10, ...observedTiers.value, ...(claimTiers.value ?? [])]),
  ].sort((a, b) => a - b),
);
const visibleClaimTiers = computed(() =>
  claimTiers.value ? new Set(claimTiers.value) : undefined,
);
const detailStatus = ref<ClaimDetailsStatus>("idle");
const {
  terrain: showTerrain,
  claims: showClaims,
  buildings: showBuildings,
  players: showPlayers,
  roads: showRoads,
  resources: showResources,
} = toRefs(layers);
const selectedClaim = shallowRef<WorldMapClaim>();
const claimPopoverHost = shallowRef<HTMLElement>();
const claimPopover = useTemplateRef<HTMLElement>("claimPopover");
const showPopoverBelow = ref(false);
const claimStatus = ref<"idle" | "loading" | "ready" | "empty" | "error">("idle");
const hasNames = computed(() => atlas.value?.regions.some((region) => region.name) ?? false);
const activeCount = computed(
  () => atlas.value?.regions.filter((region) => region.active).length ?? 0,
);
const terrainRegions = computed(
  () => atlas.value?.regions.filter((region) => region.chunks > 0).length ?? 0,
);
const number = new Intl.NumberFormat("en-US");
const coordinates = ref("");
let map: OlMap | undefined;
let claimOverlay: Overlay | undefined;
let cancel: AbortController | undefined;
let fitWorld: ((duration?: number) => void) | undefined;
let fitRegion: ((region: WorldMapRegion) => void) | undefined;
let stopOverview: (() => void) | undefined;
let invalidateRegions: (() => void) | undefined;
let invalidateClaims: (() => void) | undefined;
let setClaimsVisible: ((visible: boolean) => Promise<void>) | undefined;
let refreshClaimDetails: ((filtersChanged?: boolean, retry?: boolean) => void) | undefined;
let refreshBuildings: (() => void) | undefined;
let setPlayersVisible: ((visible: boolean, regions?: string) => void) | undefined;
let playersOverlay: ReturnType<typeof createAtlasPlayersOverlay> | undefined;
let refreshRoads: (() => void) | undefined;
let refreshResources: (() => void) | undefined;
let setResourceIcons: ReturnType<typeof createAtlasResourcesOverlay>["setIcons"] | undefined;
let setResourceColors: ReturnType<typeof createAtlasResourcesOverlay>["setColors"] | undefined;
let refreshTerrainHexes: (() => void) | undefined;
let findClaim: ((id: string) => WorldMapClaim | undefined) | undefined;
let pendingClaimId: string | undefined;
let restoredClaim: WorldMapClaim | undefined;
let restoreUrl: ((state: MapUrlState) => void) | undefined;
let readCamera: ((rendered?: boolean) => MapCamera | undefined) | undefined;
let applyingUrl = false;
let viewSaveTimer: ReturnType<typeof setTimeout> | undefined;
const contextPoint = shallowRef<{ x: number; z: number; region?: number }>();
let pointRegion: ((coordinate: number[]) => number | undefined) | undefined;
const toast = useToast();

async function loadResourceCatalog() {
  const signal = cancel?.signal;
  if (
    !currentBase ||
    !signal ||
    signal.aborted ||
    resourceCatalogStatus.value === "loading" ||
    resourceCatalogStatus.value === "ready"
  )
    return;
  resourceCatalogStatus.value = "loading";
  try {
    const types = await $fetch<AtlasResourceType[]>(`${currentBase}resource-types`, {
      // Keep pre-icon catalog responses out of the browser's five-minute cache.
      query: { v: 2 },
      signal,
      retry: 0,
      timeout: 10_000,
    });
    if (!signal.aborted) {
      resourceTypes.value = types;
      resourceCatalogStatus.value = "ready";
    }
  } catch {
    if (!signal.aborted) resourceCatalogStatus.value = "error";
  }
}

function toggleResource(id: number, enabled: boolean) {
  resourceIds.value = atlasResourceIds(
    enabled ? [...resourceIds.value, id] : resourceIds.value.filter((chosen) => chosen !== id),
  );
}

function removeLegendResource(id: number, event: MouseEvent) {
  const row = (event.currentTarget as HTMLButtonElement).closest("li");
  const next = (
    row?.nextElementSibling ?? row?.previousElementSibling
  )?.querySelector<HTMLButtonElement>(".atlas-resource-remove");
  toggleResource(id, false);
  (next ?? layersButton.value)?.focus({ preventScroll: true });
}

function clearLegendResources() {
  resourceIds.value = [];
  layersButton.value?.focus({ preventScroll: true });
}

function toggleFavoriteResource(type: Pick<AtlasResourceType, "id" | "name">) {
  monitor.toggle("favorites", { kind: "resource", id: String(type.id), name: type.name });
}

function showFavoriteResource(id: number, enabled: boolean) {
  if (
    enabled &&
    !resourceIds.value.includes(id) &&
    resourceIds.value.length >= MAX_RESOURCE_FILTERS
  )
    return;
  toggleResource(id, enabled);
  if (enabled) showResources.value = true;
}

function captureMapState(): MapUrlState {
  return {
    layers,
    claimDetails,
    buildingGroups: buildingGroups.value,
    resourceIds: resourceIds.value,
    claimTiers: claimTiers.value,
    camera: readCamera?.(),
    region: selected.value?.id,
    claim: selectedClaim.value?.id ?? pendingClaimId,
  };
}

function saveMapView() {
  if (!readCamera || applyingUrl) return false;
  try {
    localStorage.setItem(ATLAS_VIEW_KEY, JSON.stringify(writeMapUrl(captureMapState())));
    return true;
  } catch {
    return false;
  }
}

function scheduleViewSave() {
  if (!readCamera || applyingUrl) return;
  clearTimeout(viewSaveTimer);
  viewSaveTimer = setTimeout(() => {
    viewSaveTimer = undefined;
    saveMapView();
  }, 90);
}

function restoreSharedView() {
  const current = router.currentRoute.value;
  if (!restoreUrl || current.matched.at(-1) !== mapRoute) return;
  const { shared, other } = splitMapQuery(current.query);
  if (!shared) return;
  clearTimeout(viewSaveTimer);
  viewSaveTimer = undefined;
  restoreUrl(readMapUrl(current.query));
  if (saveMapView()) {
    void router.replace({ path: current.path, query: other, hash: current.hash });
  }
}

function captureContextPoint(event: MouseEvent) {
  const coordinate =
    event.clientX || event.clientY ? map?.getEventCoordinate(event) : map?.getView().getCenter();
  const [x, z] = coordinate ?? [];
  contextPoint.value =
    x === undefined || z === undefined
      ? undefined
      : {
          x,
          z,
          region: pointRegion?.([x, z]),
        };
}

async function copyMapContext(kind: "point" | "view" | "chat") {
  let text: string;
  const point = contextPoint.value;
  if (kind === "chat") {
    if (!point || !currentHasTerrain.value) return;
    text = mapChatCoordinates(point.x, point.z);
  } else {
    const state = captureMapState();
    state.camera = readCamera?.(true);
    if (kind === "point") {
      const zoom = map?.getView().getZoom();
      if (!point || zoom === undefined) return;
      state.camera = {
        space: currentHasTerrain.value ? "world" : "grid",
        x: point.x,
        z: point.z,
        zoom,
      };
      state.region = point.region;
      state.claim = undefined;
    }
    const current = router.currentRoute.value;
    const { other } = splitMapQuery(current.query);
    const { href } = router.resolve({
      path: current.path,
      query: { ...other, ...writeMapUrl(state) },
      hash: current.hash,
    });
    text = new URL(href, window.location.href).href;
  }
  try {
    await navigator.clipboard.writeText(text);
    toast.add({ title: kind === "chat" ? "Coordinates copied" : "Map link copied" });
  } catch {
    window.prompt("Copy this text:", text);
  }
}

const mapContextItems = computed(() => [
  {
    label: "Copy link to this point",
    disabled: !contextPoint.value,
    onSelect: () => copyMapContext("point"),
  },
  { label: "Copy link to current view", onSelect: () => copyMapContext("view") },
  {
    label: "Copy in-game coordinates",
    disabled: !contextPoint.value || !currentHasTerrain.value,
    onSelect: () => copyMapContext("chat"),
  },
]);

function openRequestedClaim() {
  if (!pendingClaimId || !showClaims.value) return;
  const claim = findClaim?.(pendingClaimId);
  if (!claim || !isClaimTierVisible(claim.tier)) return;
  pendingClaimId = undefined;
  if (selectedClaim.value !== claim) {
    restoredClaim = claim;
    selectedClaim.value = claim;
  }
}

function clearClaim() {
  if (pendingClaimId) {
    pendingClaimId = undefined;
    scheduleViewSave();
  }
  selectedClaim.value = undefined;
}

function reset() {
  clearClaim();
  selected.value = undefined;
  stopOverview?.();
  fitWorld?.(240);
}

function dismissClaim() {
  if (!selectedClaim.value) return;
  clearClaim();
  target.value?.focus({ preventScroll: true });
}

function isClaimTierVisible(tier: number) {
  return visibleClaimTiers.value?.has(tier) ?? true;
}

function setClaimTier(tier: number, visible: boolean) {
  const enabled = new Set(claimTiers.value ?? selectableClaimTiers.value);
  if (visible) enabled.add(tier);
  else enabled.delete(tier);
  claimTiers.value =
    enabled.size === selectableClaimTiers.value.length
      ? undefined
      : [...enabled].sort((a, b) => a - b);
}

function makeClaimLayer(scale: AtlasMarkerScale) {
  const styles = new Map<string, Style>();
  const layer = new VectorLayer({
    source: new VectorSource<Feature<Point>>({ wrapX: false }),
    visible: false,
    declutter: "claims",
    updateWhileInteracting: true,
    renderBuffer: 64,
    renderOrder: (a, b) => compareClaimMarkerPriority(a.get("claim"), b.get("claim")),
    style: (feature, resolution) => {
      const claim = feature.get("claim") as WorldMapClaim;
      if (disabledRegionIds.value.has(claim.region) || !isClaimTierVisible(claim.tier)) return;
      const selected = selectedClaim.value?.id === claim.id;
      const { radius, fontSize } = claimBadgeSize(resolution);
      const key = `${claim.tier}/${selected}/${radius}`;
      let style = styles.get(key);
      if (!style) {
        style = new Style({
          image: new CircleStyle({
            radius,
            fill: new Fill({ color: selected ? "#f0c78c" : "#45351f" }),
            stroke: new Stroke({ color: "#ffdb9a", width: selected ? 2 : 1 }),
          }),
          text: new Text({
            text: String(claim.tier),
            font: `700 ${fontSize}px system-ui, sans-serif`,
            fill: new Fill({ color: selected ? "#2f2215" : "#fff2dc" }),
          }),
        });
        styles.set(key, style);
      }
      return scale.apply(style);
    },
  });
  scale.track(layer);
  return layer;
}

function zoomBy(amount: number) {
  if (!map) return;
  stopOverview?.();
  const view = map.getView();
  view.animate({ zoom: view.getConstrainedZoom((view.getZoom() ?? 0) + amount), duration: 180 });
}

function locateTrackedPlayer(id: string) {
  const center = playersOverlay?.trackedPosition(id);
  if (!center || !map) return;
  stopOverview?.();
  const view = map.getView();
  view.cancelAnimations();
  view.animate({ center, resolution: Math.min(view.getResolution() ?? 4, 4), duration: 240 });
}

function regionStyle(
  region: WorldMapRegion,
  schematic: boolean,
  selected: boolean,
  grid: boolean,
  numberLabel: boolean,
  nameLabel: boolean,
  detail: boolean,
) {
  const style = new Style({
    fill: new Fill({
      color: !schematic ? "transparent" : region.active ? "#254b53" : "#182b36",
    }),
    stroke: grid
      ? new Stroke({
          color: selected
            ? "#f0c78c"
            : detail
              ? "rgba(220, 234, 232, 0.42)"
              : "rgba(220, 234, 232, 0.30)",
          width: selected ? 2 : 1,
        })
      : undefined,
  });
  const labelStyle = (text: string, offsetY: number, font: string) =>
    new Style({
      text: new Text({
        text,
        offsetY,
        font,
        padding: [3, 5, 3, 5],
        fill: new Fill({ color: region.active === false && !selected ? "#8ea6b0" : "#fff0d2" }),
        backgroundFill: new Fill({ color: "#102531" }),
        backgroundStroke: new Stroke({ color: selected ? "#f0c78c" : "#59717a", width: 1 }),
      }),
    });
  const styles = [style];
  if (numberLabel) {
    styles.push(
      labelStyle(
        String(region.id).padStart(2, "0"),
        nameLabel ? -12 : 0,
        "600 13px ui-monospace, monospace",
      ),
    );
  }
  if (nameLabel && region.name) {
    styles.push(labelStyle(region.name, numberLabel ? 12 : 0, "600 13px system-ui, sans-serif"));
  }
  return styles;
}

async function load() {
  if (!target.value) return;
  if (!preferencesLoaded) {
    preferencesLoaded = true;
    try {
      const preferences = readAtlasPreferences(localStorage.getItem(ATLAS_PREFERENCES_KEY));
      disabledRegions.value = preferences.disabledRegions;
      showResourceIcons.value = preferences.showResourceIcons;
      showPlayerNames.value = preferences.showPlayerNames;
      displaySizes.terrainSaturation = preferences.terrainSaturation;
      displaySizes.pavingSaturation = preferences.pavingSaturation;
      showGrid.value = preferences.showGrid;
      showRegionNumbers.value = preferences.showRegionNumbers;
      showRegionNames.value = preferences.showRegionNames;
      resourceColors.value = preferences.resourceColors;
      displaySizes.iconSize = preferences.iconSize;
      displaySizes.infoSize = preferences.infoSize;
    } catch {
      // The atlas still works if browser storage is blocked.
    }
  }
  cancel?.abort();
  map?.dispose();
  map = undefined;
  claimOverlay = claimPopoverHost.value = undefined;
  fitWorld = fitRegion = stopOverview = undefined;
  invalidateRegions = invalidateClaims = setClaimsVisible = undefined;
  refreshClaimDetails = undefined;
  refreshBuildings = undefined;
  setPlayersVisible = undefined;
  playersOverlay = undefined;
  visibleTrackedPlayers.value = new Set();
  refreshRoads = undefined;
  refreshResources = undefined;
  setResourceIcons = undefined;
  setResourceColors = undefined;
  refreshTerrainHexes = undefined;
  findClaim = restoreUrl = readCamera = undefined;
  pendingClaimId = restoredClaim = undefined;
  selected.value = atlas.value = undefined;
  selectedClaim.value = undefined;
  claimStatus.value = "idle";
  detailStatus.value = "idle";
  buildingsStatus.value = "idle";
  atlasPlayersStatus.value = "off";
  atlasRoadsStatus.value = "off";
  resourcesStatus.value = "off";
  if (resourceCatalogStatus.value === "loading") resourceCatalogStatus.value = "idle";
  onlinePlayerCount.value = 0;
  observedTiers.value = [];
  coordinates.value = "";
  currentHasTerrain.value = false;
  const request = new AbortController();
  cancel = request;
  const source = currentBase;
  let raster: WorldMapAtlas | undefined;
  if (source) {
    try {
      raster = await $fetch<WorldMapAtlas>(`${source}index.json`, {
        signal: request.signal,
        retry: 0,
        timeout: 3500,
      });
      if (
        !raster.regions?.length ||
        !raster.resolutions?.length ||
        raster.tiles?.length !== raster.resolutions.length
      )
        throw new Error("Invalid world atlas");
    } catch {
      raster = undefined;
    }
  }
  if (request.signal.aborted || !target.value) return;
  const schematic = !raster;
  currentHasTerrain.value = !schematic;
  const data =
    raster?.regions.length === CURRENT_WORLD_LAYOUT.regions.length
      ? {
          ...raster,
          regions: raster.regions.map((region) => ({
            ...region,
            active: region.active || CURRENT_WORLD_LAYOUT.regions[region.id - 1]?.active,
          })),
        }
      : (raster ?? CURRENT_WORLD_LAYOUT);
  atlas.value = data;
  const bounds = schematic ? schematicRegionExtent : regionExtent;
  const regionBounds = data.regions.map((region) => ({ id: region.id, extent: bounds(region) }));
  pointRegion = (coordinate) =>
    regionBounds.find(({ extent }) => containsCoordinate(extent, coordinate))?.id;
  const world = data.regions.reduce(
    (extent, region) => extend(extent, bounds(region)),
    createEmpty(),
  );
  const projection = new Projection({
    code: schematic ? "BITCRAFT:REGION_GRID" : "BITCRAFT:OVERWORLD",
    units: schematic ? "pixels" : "m",
    extent: raster?.extent ?? world,
  });
  const terrain = raster && source ? terrainLayer(raster, projection, source) : undefined;
  const scale = createAtlasMarkerScale(iconFactor.value);
  const labels = createAtlasMarkerScale(infoFactor.value);
  markerScale = scale;
  labelScale = labels;
  const claims = terrain ? makeClaimLayer(scale) : undefined;
  if (claims) {
    const host = document.createElement("div");
    claimPopoverHost.value = host;
    claimOverlay = new Overlay({ element: host, positioning: "bottom-center" });
  }
  const features = data.regions.map(
    (region) => new Feature({ geometry: fromExtent(bounds(region)), region }),
  );
  const styles = new Map<string, Style[]>();
  const regions = new VectorLayer({
    source: new VectorSource({ features, wrapX: false }),
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    renderBuffer: 64,
    style: (feature, resolution) => {
      const region = feature.get("region") as WorldMapRegion;
      const active = selected.value?.id === region.id;
      const grid = showGrid.value;
      const numberLabel = showRegionNumbers.value;
      const nameLabel = showRegionNames.value && !!region.name;
      const detail = !schematic && resolution < 12;
      const key = `${region.id}/${active}/${grid}/${numberLabel}/${nameLabel}/${detail}`;
      let style = styles.get(key);
      if (!style) {
        style = regionStyle(region, schematic, active, grid, numberLabel, nameLabel, detail);
        styles.set(key, style);
      }
      style.forEach(labels.apply);
      return style;
    },
  });
  labels.track(regions);
  invalidateRegions = () => regions.changed();
  if (claims && source) {
    let claimScope: string | undefined;
    let claimRequest: AbortController | undefined;
    request.signal.addEventListener("abort", () => claimRequest?.abort(), { once: true });
    let requestedId: string | undefined;
    invalidateClaims = () => claims.changed();
    findClaim = (id) =>
      claims.getSource()?.getFeatureById(id)?.get("claim") as WorldMapClaim | undefined;
    setClaimsVisible = async (visible) => {
      claims.setVisible(visible);
      const scope = regionScope.value;
      if (scope !== claimScope) {
        claimScope = scope;
        claimRequest?.abort();
        claims.getSource()?.clear(true);
        claimStatus.value = "idle";
      }
      if (!visible) return;
      const claim = pendingClaimId && !findClaim?.(pendingClaimId) ? pendingClaimId : undefined;
      if (claimStatus.value === "loading" && requestedId === claim) return;
      if (!claim && claimStatus.value !== "idle" && claimStatus.value !== "error") return;
      if (!claim && scope === "none") {
        claimStatus.value = "empty";
        return;
      }
      claimRequest?.abort();
      const controller = new AbortController();
      const { signal } = controller;
      claimRequest = controller;
      requestedId = claim;
      claimStatus.value = "loading";
      try {
        const results = await $fetch<WorldMapClaim[]>(`${source}claims`, {
          query: { regions: scope, claim },
          signal,
          retry: 0,
          timeout: 10_000,
        });
        if (signal.aborted) return;
        if (!Array.isArray(results)) throw new Error("Invalid claim markers");
        observedTiers.value = [...new Set(results.map((claim) => claim.tier))];
        const features = results
          .filter((row) => row.id === claim || !disabledRegionIds.value.has(row.region))
          .map((claim) => {
            const feature = new Feature({
              geometry: new Point(smallHexCenter(claim.x, claim.z)),
              claim,
            });
            feature.setId(claim.id);
            return feature;
          });
        claims.getSource()?.clear(true);
        claims.getSource()?.addFeatures(features);
        claimStatus.value = features.length ? "ready" : "empty";
        openRequestedClaim();
      } catch {
        if (!signal.aborted) claimStatus.value = "error";
      }
    };
  }
  const resolution = raster?.resolutions[0] ?? getWidth(world) / 512;
  const finestTerrainResolution = raster?.resolutions.at(-1);
  const view = new View({
    projection,
    center: getCenter(world),
    maxResolution: resolution * 4,
    minResolution:
      finestTerrainResolution === undefined
        ? resolution / 2
        : finestTerrainResolution <= TERRAIN_HEX_MAX_SOURCE_RESOLUTION
          ? TERRAIN_MIN_RESOLUTION
          : finestTerrainResolution / 4,
    resolution,
    extent: buffer(world, getWidth(world) * 0.14),
    showFullExtent: true,
    enableRotation: false,
  });
  const canvas = new OlMap({
    target: target.value,
    layers: [...(terrain ? [terrain] : []), regions, ...(claims ? [claims] : [])],
    overlays: claimOverlay ? [claimOverlay] : [],
    view,
    controls: controls({ attribution: false, rotate: false, zoom: false }),
    interactions: interactions({ altShiftDragRotate: false, pinchRotate: false }),
    maxTilesLoading: ATLAS_MAX_TILES_LOADING,
  });
  if (terrain) enableAtlasZoomTileLoading(canvas, () => terrain.getVisible());
  map = canvas;
  let resourceOverlay: ReturnType<typeof createAtlasResourcesOverlay> | undefined;
  if (terrain && raster && source) {
    refreshTerrainHexes = createTerrainHexOverlay(
      canvas,
      raster,
      source,
      request.signal,
      () => showTerrain.value,
      () => terrainSaturation.value,
    );
    refreshRoads = createAtlasRoadsOverlay(
      canvas,
      canvas.getLayers().getArray().indexOf(regions),
      request.signal,
      () => showRoads.value,
      (status) => {
        atlasRoadsStatus.value = status;
      },
      cachedMapQuery(`${source}roads`),
      () => regionScope.value,
      () => pavingSaturation.value,
    );
    resourceOverlay = createAtlasResourcesOverlay(
      canvas,
      canvas.getLayers().getArray().indexOf(regions),
      raster.regions,
      request.signal,
      () => ({
        enabled: showResources.value,
        ids: resourceIds.value,
        disabledRegions: disabledRegionIds.value,
      }),
      (status) => {
        resourcesStatus.value = status;
      },
      cachedMapQuery(`${source}resources`),
      cachedMapQuery(`${source}resource-clusters`),
      scale,
    );
    refreshResources = resourceOverlay.refresh;
    setResourceIcons = resourceOverlay.setIcons;
    setResourceIcons(resourceIcons.value);
    setResourceColors = resourceOverlay.setColors;
    setResourceColors(resourceColors.value);
  }
  if (claims && source) {
    refreshClaimDetails = createClaimDetailsOverlay(
      canvas,
      request.signal,
      () => ({ ...claimDetails, enabled: showClaims.value }),
      (status) => {
        detailStatus.value = status;
      },
      cachedMapQuery(`${source}claim-details`),
      () => regionScope.value,
      scale,
    );
    refreshBuildings = createMapBuildingsOverlay(
      canvas,
      request.signal,
      () =>
        showBuildings.value
          ? MAP_BUILDING_GROUPS.filter((group) => buildingGroups.value.includes(group))
          : [],
      (status) => {
        buildingsStatus.value = status;
      },
      (query, signal) => $fetch(`${source}buildings`, { query, signal, retry: 0, timeout: 10_000 }),
      () => regionScope.value,
      scale,
    );
  }
  if (!schematic) {
    playersOverlay = createAtlasPlayersOverlay(
      canvas,
      config.public.api.websocket,
      request.signal,
      (status, count) => {
        atlasPlayersStatus.value = status;
        onlinePlayerCount.value = count;
      },
      undefined,
      scale,
      labels,
      (ids) => {
        visibleTrackedPlayers.value = ids;
      },
      source
        ? (ids, signal) =>
            $fetch(`${source}player-names`, {
              query: { ids: ids.join(",") },
              signal,
              timeout: 5_000,
              retry: 0,
            })
        : undefined,
    );
    setPlayersVisible = playersOverlay.setEnabled;
  }
  if (!schematic) {
    const viewport = canvas.getViewport();
    let pointer: PointerEvent | undefined;
    const updatePointerCoordinates = () => {
      if (!pointer) return;
      const [x, z] = canvas.getEventCoordinate(pointer) ?? [];
      if (x === undefined || z === undefined) return;
      const { north, east } = mapGameCoordinates(x, z);
      coordinates.value = `N ${number.format(north)} · E ${number.format(east)}`;
    };
    viewport.addEventListener(
      "pointermove",
      (event) => {
        if (event.pointerType === "touch") return;
        pointer = event;
        updatePointerCoordinates();
      },
      { passive: true, signal: request.signal },
    );
    viewport.addEventListener(
      "pointerleave",
      () => {
        pointer = undefined;
        coordinates.value = "";
      },
      { signal: request.signal },
    );
    // Keep the coordinates under a stationary cursor accurate while the view moves.
    canvas.on("postrender", updatePointerCoordinates);
  }
  canvas.on("moveend", () => {
    scheduleViewSave();
    refreshTerrainHexes?.();
    refreshClaimDetails?.();
    refreshBuildings?.();
    refreshRoads?.();
    refreshResources?.();
  });
  const claimAtPixel = (pixel: Pixel) => {
    if (!showClaims.value || !claims) return;
    return canvas.forEachFeatureAtPixel(pixel, (feature) => feature.get("claim"), {
      layerFilter: (layer) => layer === claims,
      hitTolerance: 3,
    }) as WorldMapClaim | undefined;
  };
  canvas.on("click", (event) => {
    const claim = claimAtPixel(event.pixel);
    if (claim) {
      if (selectedClaim.value?.id === claim.id) dismissClaim();
      else {
        pendingClaimId = undefined;
        selectedClaim.value = claim;
      }
      return;
    }
    clearClaim();
    if (resourceOverlay?.clusterAtPixel(event.pixel)) return;
    const region = canvas.forEachFeatureAtPixel(event.pixel, (feature) => feature.get("region"), {
      layerFilter: (layer) => layer === regions,
    }) as WorldMapRegion | undefined;
    if (region) selected.value = region;
  });
  canvas.on("singleclick", (event) => {
    if (claimAtPixel(event.pixel)) return;
    const cluster = resourceOverlay?.clusterAtPixel(event.pixel);
    if (!cluster) return;
    stopOverview?.();
    resourceOverlay?.zoomToCluster(cluster);
  });
  // Two quick clicks on a badge toggle its popover, not the map's zoom.
  canvas.on("dblclick", (event) => {
    if (claimAtPixel(event.pixel) || resourceOverlay?.clusterAtPixel(event.pixel)) return false;
  });
  const space = schematic ? "grid" : "world";
  let otherSpaceCamera: MapCamera | undefined;
  readCamera = (rendered = false) => {
    if (otherSpaceCamera && !rendered) return otherSpaceCamera;
    const [x, z] = view.getCenter() ?? [];
    const zoom = view.getZoom();
    if (x !== undefined && z !== undefined && zoom !== undefined) {
      return { space, x, z, zoom };
    }
  };
  let overview = true;
  stopOverview = () => {
    overview = false;
    otherSpaceCamera = undefined;
  };
  fitWorld = (duration = 0) => {
    overview = true;
    view.fit(world, { padding: [28, 24, 28, 24], duration });
  };
  fitRegion = (region) => {
    clearClaim();
    selected.value = region;
    stopOverview?.();
    view.fit(bounds(region), { padding: [36, 32, 36, 32], duration: 300 });
  };
  canvas.on("pointerdrag", stopOverview);
  target.value.addEventListener("wheel", stopOverview, { passive: true, signal: request.signal });
  target.value.addEventListener("keydown", stopOverview, { signal: request.signal });
  canvas.on("change:size", () => {
    if (overview && canvas.getSize()?.every((size) => size > 60)) fitWorld?.();
  });
  restoreUrl = (state) => {
    applyingUrl = true;
    try {
      Object.assign(layers, state.layers);
      Object.assign(claimDetails, state.claimDetails);
      buildingGroups.value = state.buildingGroups;
      resourceIds.value = state.resourceIds;
      claimTiers.value = state.claimTiers;
      selected.value = data.regions.find((region) => region.id === state.region);
      pendingClaimId = state.claim;
      if (selectedClaim.value?.id !== state.claim) selectedClaim.value = undefined;
      openRequestedClaim();
      if (pendingClaimId && showClaims.value) void setClaimsVisible?.(true);

      view.cancelAnimations();
      const camera = state.camera;
      // If terrain is unavailable, don't replace a shared world position with
      // schematic grid coordinates until the user actually moves that map.
      otherSpaceCamera = camera && camera.space !== space ? camera : undefined;
      if (camera?.space === space) {
        overview = false;
        view.setZoom(view.getConstrainedZoom(camera.zoom));
        view.setCenter(view.getConstrainedCenter([camera.x, camera.z]));
      } else {
        fitWorld?.();
      }
    } finally {
      applyingUrl = false;
    }
  };
  if (splitMapQuery(router.currentRoute.value.query).shared) restoreSharedView();
  else restoreUrl(readCurrentMapState());
  window.addEventListener("pagehide", saveMapView, { signal: request.signal });
  if (showClaims.value) void setClaimsVisible?.(true);
  refreshTerrainHexes?.();
  refreshClaimDetails?.();
  refreshBuildings?.();
  setPlayersVisible?.(showPlayers.value, regionScope.value);
  playersOverlay?.setTracking(monitorTargets.value);
  playersOverlay?.setColors(monitorPreferences.value.playerColors);
  playersOverlay?.setShowNames(showPlayerNames.value);
  refreshRoads?.();
  refreshResources?.();
}

function terrainLayer(data: WorldMapAtlas, projection: Projection, base: string) {
  const tileSets = data.tiles.map((keys) => new Set(keys));
  const layer = new TileLayer({
    className: "ol-layer atlas-terrain-layer",
    visible: showTerrain.value,
    cacheSize: 96,
    preload: 1,
    source: new XYZ({
      projection,
      wrapX: false,
      // Don't leave a fully downloaded tile mostly transparent for 250 ms.
      transition: 80,
      tileGrid: new TileGrid({
        extent: data.extent,
        tileSize: data.tileSize,
        resolutions: data.resolutions,
      }),
      tileUrlFunction: ([z, x, y]) => {
        if (z === undefined || x === undefined || y === undefined) return;
        if (tileSets[z]?.has(atlasTileKey(z, x, y)))
          return `${base}${data.version}/${z}/${x}/${y}.webp`;
      },
    }),
  });
  // Scope the filter to tile draws. A CSS filter on the moving layer canvas
  // can leave Chromium's composited surface blank or stale during updates.
  let previousFilter: string | undefined;
  layer.on("prerender", ({ context }) => {
    if (terrainSaturation.value === 1 || !context || !("filter" in context)) return;
    previousFilter = context.filter;
    context.filter = `saturate(${terrainSaturation.value})`;
  });
  layer.on("postrender", ({ context }) => {
    if (previousFilter !== undefined && context && "filter" in context)
      context.filter = previousFilter;
    previousFilter = undefined;
  });
  return layer;
}

watch(terrainSaturation, () => {
  map?.getLayers().forEach((layer) => {
    if (layer.getClassName() === "ol-layer atlas-terrain-layer") layer.changed();
  });
});
watch(pavingSaturation, () => {
  map?.getLayers().forEach((layer) => {
    if (layer.getClassName() === "ol-layer atlas-paving-layer") layer.changed();
  });
});
watch(resourceIcons, (icons) => setResourceIcons?.(icons));
watch(resourceColors, (colors) => setResourceColors?.(colors));
watch(iconFactor, (factor) => markerScale?.set(factor));
watch(infoFactor, (factor) => labelScale?.set(factor));
watch(
  [
    disabledRegions,
    showResourceIcons,
    showPlayerNames,
    () => displaySizes.terrainSaturation,
    () => displaySizes.pavingSaturation,
    showGrid,
    showRegionNumbers,
    showRegionNames,
    resourceColors,
    () => displaySizes.iconSize,
    () => displaySizes.infoSize,
  ],
  ([
    disabledRegions,
    showResourceIcons,
    showPlayerNames,
    terrainSaturation,
    pavingSaturation,
    showGrid,
    showRegionNumbers,
    showRegionNames,
    resourceColors,
    iconSize,
    infoSize,
  ]) => {
    try {
      localStorage.setItem(
        ATLAS_PREFERENCES_KEY,
        JSON.stringify({
          disabledRegions,
          showResourceIcons,
          showPlayerNames,
          terrainSaturation,
          pavingSaturation,
          showGrid,
          showRegionNumbers,
          showRegionNames,
          resourceColors,
          iconSize,
          infoSize,
        }),
      );
    } catch {
      // Keep the current preferences in memory when storage is unavailable.
    }
  },
);
watch(disabledRegionIds, () => {
  invalidateClaims?.();
  void setClaimsVisible?.(showClaims.value);
  refreshClaimDetails?.();
  refreshBuildings?.();
  refreshRoads?.();
  refreshResources?.();
  setPlayersVisible?.(showPlayers.value, regionScope.value);
});
watch(
  showTerrain,
  (visible) => {
    const layer = map?.getLayers().item(0);
    if (layer instanceof TileLayer) layer.setVisible(visible);
    refreshTerrainHexes?.();
  },
  { flush: "sync" },
);
watch(
  [selected, showGrid, showRegionNumbers, showRegionNames],
  () => {
    invalidateRegions?.();
  },
  { flush: "sync" },
);
watch(showClaims, (visible) => {
  if (!visible) clearClaim();
  void setClaimsVisible?.(visible);
});
watch(
  () => [showClaims.value, ...CLAIM_DETAIL_KEYS.map((key) => claimDetails[key])],
  () => refreshClaimDetails?.(true),
);
watch(
  claimTiers,
  () => {
    invalidateClaims?.();
    if (applyingUrl) return;
    if (selectedClaim.value && !isClaimTierVisible(selectedClaim.value.tier)) clearClaim();
    openRequestedClaim();
  },
  { flush: "sync" },
);
watch(claimDetails, scheduleViewSave, { flush: "sync" });
watch([showBuildings, buildingGroups], () => refreshBuildings?.());
watch(showPlayers, (visible) => setPlayersVisible?.(visible, regionScope.value));
watch(monitorTargets, (targets) => playersOverlay?.setTracking(targets));
watch(showPlayerNames, (visible) => playersOverlay?.setShowNames(visible));
watch(
  () => monitorPreferences.value.playerColors,
  (colors) => playersOverlay?.setColors(colors),
);
watch(showRoads, () => refreshRoads?.());
watch([showResources, resourceIds], () => refreshResources?.());
watch([showResources, openLayerOptions, currentHasTerrain], ([enabled, menu, available]) => {
  if (available && (enabled || menu === "resources")) void loadResourceCatalog();
});
watch(
  [
    showTerrain,
    showClaims,
    showBuildings,
    showPlayers,
    showRoads,
    showResources,
    resourceIds,
    buildingGroups,
    claimTiers,
    selected,
    selectedClaim,
  ],
  scheduleViewSave,
  { flush: "sync" },
);
watch(() => router.currentRoute.value.fullPath, restoreSharedView);
watch(selectedClaim, () => invalidateClaims?.(), { flush: "sync" });
watch(
  selectedClaim,
  (claim) => {
    const fromUrl = restoredClaim === claim;
    restoredClaim = undefined;
    const overlay = claimOverlay;
    if (!overlay || !map) return;
    if (!claim) {
      overlay.setPosition(undefined);
      return;
    }

    const coordinate = smallHexCenter(claim.x, claim.z);
    overlay.setPosition(coordinate);
    const y = map.getPixelFromCoordinate(coordinate)?.[1];
    const height = map.getSize()?.[1];
    const cardHeight = claimPopover.value?.offsetHeight ?? 0;
    showPopoverBelow.value =
      y !== undefined && height !== undefined && y < cardHeight + 30 && y < height / 2;
    overlay.setPositioning(showPopoverBelow.value ? "top-center" : "bottom-center");
    overlay.setOffset([0, (showPopoverBelow.value ? 18 : -18) * iconFactor.value]);
    if (!fromUrl) {
      overlay.panIntoView({ margin: 14, animation: { duration: 180 } });
      claimPopover.value?.focus({ preventScroll: true });
    }
  },
  { flush: "post" },
);
watch(iconFactor, (factor) => {
  if (!claimOverlay || !selectedClaim.value) return;
  claimOverlay.setOffset([0, (showPopoverBelow.value ? 18 : -18) * factor]);
});
// Nuxt's client-only page can mount before its inner template is rendered.
watch(
  target,
  (element) => {
    if (element && !map) void load();
  },
  { flush: "post" },
);
onBeforeUnmount(() => {
  clearTimeout(viewSaveTimer);
  if (viewSaveTimer !== undefined) saveMapView();
  cancel?.abort();
  map?.dispose();
  map = undefined;
  claimOverlay = undefined;
  markerScale = undefined;
  labelScale = undefined;
});
</script>

<template>
  <div class="atlas-shell" :style="[oceanBackdropStyle, { '--atlas-info-scale': infoFactor }]">
    <h1 class="sr-only">World Atlas</h1>
    <section class="atlas-frame" aria-label="Interactive overworld map">
      <UContextMenu :items="mapContextItems" :disabled="!atlas" :modal="false">
        <div
          ref="target"
          tabindex="0"
          aria-label="Overworld map. Drag to pan. Scroll to zoom. Right-click for sharing options."
          class="atlas-map"
          @contextmenu.capture="captureContextPoint"
          @pointerdown.capture="
            ($event.pointerType === 'touch' || $event.pointerType === 'pen') &&
            captureContextPoint($event)
          "
          @keydown.esc.stop.prevent="dismissClaim"
        />
      </UContextMenu>
      <div v-if="atlas" class="atlas-controls">
        <button type="button" title="Zoom in" aria-label="Zoom in" @click="zoomBy(1)">+</button>
        <button type="button" title="Zoom out" aria-label="Zoom out" @click="zoomBy(-1)">−</button>
        <button type="button" title="Fit world" aria-label="Fit world" @click="reset">⌖</button>
      </div>
      <div v-if="atlas" class="atlas-north" aria-hidden="true"><span>▲</span>N</div>
      <div v-if="atlas" class="atlas-coordinate">
        <span>{{ selected ? selected.name || `Region ${selected.id}` : "Overworld" }}</span>
        <span v-if="selected?.active !== undefined">{{
          selected.active ? "Active" : "Inactive"
        }}</span>
        <span v-if="selected && currentHasTerrain">{{ regionCoverage(selected) }}</span>
        <span v-if="selected && disabledRegionIds.has(selected.id)">Overlay data off</span>
        <span v-if="coordinates">{{ coordinates }}</span>
      </div>
      <p v-else class="atlas-status" role="status">Loading the Overworld…</p>
      <Teleport v-if="claimPopoverHost && selectedClaim" :to="claimPopoverHost">
        <div
          ref="claimPopover"
          role="dialog"
          aria-labelledby="atlas-claim-title"
          tabindex="-1"
          class="atlas-claim-popover"
          :class="{ 'atlas-claim-popover-below': showPopoverBelow }"
          @keydown.esc.stop.prevent="dismissClaim"
        >
          <div class="atlas-claim-card">
            <div class="atlas-claim-heading">
              <div>
                <p class="atlas-eyebrow">
                  Tier {{ selectedClaim.tier }} · Region {{ selectedClaim.region }}
                </p>
                <h2 id="atlas-claim-title">{{ selectedClaim.name }}</h2>
              </div>
              <button type="button" aria-label="Close claim details" @click="dismissClaim">
                ×
              </button>
            </div>
            <dl class="atlas-claim-stats">
              <div>
                <dt>Members</dt>
                <dd>{{ number.format(selectedClaim.members) }}</dd>
              </div>
              <div>
                <dt>Claim tiles</dt>
                <dd>{{ number.format(selectedClaim.tiles) }}</dd>
              </div>
            </dl>
            <NuxtLink :to="`/claims/${selectedClaim.id}`" class="atlas-claim-link"
              >View claim <span aria-hidden="true">→</span></NuxtLink
            >
          </div>
        </div>
      </Teleport>
    </section>

    <div class="atlas-layer-stack">
      <details name="atlas-menu" class="atlas-menu atlas-layer-menu" @toggle="onLayerMenuToggle">
        <summary ref="layersButton" class="atlas-menu-button">Layers</summary>
        <div class="atlas-panel">
          <fieldset class="atlas-layer-options">
            <legend class="atlas-eyebrow">Show on map</legend>
            <label
              ><input v-model="showTerrain" type="checkbox" :disabled="!terrainRegions" />
              Terrain</label
            >
            <label>
              <input v-model="showPlayers" type="checkbox" :disabled="!currentHasTerrain" />
              Online players
            </label>
            <label>
              <input v-model="showRoads" type="checkbox" :disabled="!currentHasTerrain" />
              Paving / roads
            </label>
          </fieldset>
          <div v-if="showRoads" class="atlas-claim-status" aria-live="polite">
            <p v-if="atlasRoadsStatus === 'zoom'">Zoom in to see paving.</p>
            <p v-else-if="atlasRoadsStatus === 'loading'">Loading nearby paving…</p>
            <p v-else-if="atlasRoadsStatus === 'overview'">
              Showing simplified paving. Zoom in for individual tiles.
            </p>
            <p v-else-if="atlasRoadsStatus === 'truncated'">
              Showing paving nearest the center. Zoom in for more detail.
            </p>
            <p v-else-if="atlasRoadsStatus === 'error'">
              Couldn’t load paving.
              <button type="button" @click="refreshRoads?.()">Retry</button>
            </p>
          </div>
          <div v-if="showPlayers" class="atlas-claim-status" aria-live="polite">
            <p v-if="atlasPlayersStatus === 'live'">
              <span class="atlas-player-dot" aria-hidden="true"></span>
              {{ number.format(onlinePlayerCount) }} live
              {{ regionScope ? "in enabled regions" : "in the overworld" }}
            </p>
            <p v-else-if="atlasPlayersStatus === 'connecting'">Connecting to live players…</p>
            <p v-else-if="atlasPlayersStatus === 'reconnecting'">Reconnecting to live players…</p>
          </div>
          <AtlasLayerGroup
            v-model="showClaims"
            label="Claims"
            :disabled="!currentHasTerrain"
            :open="openLayerOptions === 'claims'"
            @update:open="setLayerOptionsOpen('claims', $event)"
          >
            <fieldset class="atlas-layer-options atlas-claim-options">
              <legend class="atlas-eyebrow">Claim details</legend>
              <label><input v-model="claimDetails.border" type="checkbox" /> Claim borders</label>
              <label v-for="kind in CLAIM_BUILDING_KINDS" :key="kind">
                <input v-model="claimDetails[kind]" type="checkbox" />
                <span
                  class="atlas-building-key"
                  :style="{ color: CLAIM_BUILDING_STYLES[kind].color }"
                  aria-hidden="true"
                  >{{ CLAIM_BUILDING_STYLES[kind].badge }}</span
                >
                {{ CLAIM_BUILDING_STYLES[kind].label }}
              </label>
              <p v-if="detailStatus === 'zoom'" class="atlas-note">
                Zoom in to see enabled details. Building markers appear before borders.
              </p>
              <p v-else-if="detailStatus === 'loading'" class="atlas-note" role="status">
                Loading nearby claim details…
              </p>
              <p v-else-if="detailStatus === 'error'" class="atlas-note atlas-claim-status">
                Couldn’t load claim details.
                <button type="button" @click="refreshClaimDetails?.(false, true)">Retry</button>
              </p>
            </fieldset>
            <fieldset class="atlas-layer-options atlas-claim-options">
              <legend class="atlas-eyebrow">Claim tiers</legend>
              <div class="atlas-tier-options">
                <label v-for="tier in selectableClaimTiers" :key="tier">
                  <input
                    type="checkbox"
                    :aria-label="`Claim tier ${tier}`"
                    :checked="isClaimTierVisible(tier)"
                    @change="setClaimTier(tier, ($event.target as HTMLInputElement).checked)"
                  />
                  {{ tier }}
                </label>
              </div>
              <div class="atlas-tier-actions">
                <button
                  type="button"
                  aria-label="Show all claim tiers"
                  @click="claimTiers = undefined"
                >
                  All
                </button>
                <button type="button" aria-label="Hide all claim tiers" @click="claimTiers = []">
                  None
                </button>
              </div>
            </fieldset>
          </AtlasLayerGroup>
          <AtlasLayerGroup
            v-model="showBuildings"
            label="Buildings"
            :disabled="!currentHasTerrain"
            :open="openLayerOptions === 'buildings'"
            @update:open="setLayerOptionsOpen('buildings', $event)"
          >
            <fieldset class="atlas-layer-options atlas-claim-options">
              <legend class="atlas-eyebrow">Building groups</legend>
              <label v-for="group in MAP_BUILDING_GROUPS" :key="group">
                <input v-model="buildingGroups" type="checkbox" :value="group" />
                <span
                  class="atlas-building-key"
                  :style="{ color: MAP_BUILDING_STYLES[group].color }"
                  aria-hidden="true"
                  >{{ MAP_BUILDING_STYLES[group].badge }}</span
                >
                {{ MAP_BUILDING_STYLES[group].label }}
              </label>
              <div class="atlas-tier-actions">
                <button
                  type="button"
                  aria-label="Show all building groups"
                  @click="buildingGroups = [...MAP_BUILDING_GROUPS]"
                >
                  All
                </button>
                <button
                  type="button"
                  aria-label="Hide all building groups"
                  @click="buildingGroups = []"
                >
                  None
                </button>
              </div>
            </fieldset>
            <p v-if="buildingsStatus === 'zoom'" class="atlas-note">
              Zoom in to see buildings and their tiers.
            </p>
            <p v-else-if="buildingsStatus === 'loading'" class="atlas-note" role="status">
              Loading nearby buildings…
            </p>
            <p v-else-if="buildingsStatus === 'truncated'" class="atlas-note" role="status">
              Too many buildings in this area. Zoom in or select fewer groups.
            </p>
            <p v-else-if="buildingsStatus === 'error'" class="atlas-note atlas-claim-status">
              Couldn’t load buildings.
              <button type="button" @click="refreshBuildings?.()">Retry</button>
            </p>
          </AtlasLayerGroup>
          <AtlasLayerGroup
            v-model="showResources"
            label="Resources"
            :disabled="!currentHasTerrain"
            :open="openLayerOptions === 'resources'"
            @update:open="setLayerOptionsOpen('resources', $event)"
          >
            <div class="atlas-layer-options atlas-resource-options">
              <input
                v-model="resourceSearch"
                type="search"
                class="atlas-resource-search"
                placeholder="Name, category, or tier"
                aria-label="Search resources"
              />
              <div class="atlas-tier-actions">
                <span>{{ resourceIds.length }} / {{ MAX_RESOURCE_FILTERS }} selected</span>
                <button v-if="resourceIds.length" type="button" @click="resourceIds = []">
                  Clear
                </button>
              </div>
              <p v-if="resourceCatalogStatus === 'loading'" class="atlas-note" role="status">
                Loading resource types…
              </p>
              <p
                v-else-if="resourceCatalogStatus === 'error'"
                class="atlas-note atlas-claim-status"
              >
                Couldn’t load resource types.
                <button type="button" @click="loadResourceCatalog">Retry</button>
              </p>
              <p v-else-if="!resourceChoices.options.length" class="atlas-note">
                No matching resource types.
              </p>
              <div
                v-for="type in resourceChoices.options"
                :key="type.id"
                class="atlas-resource-choice"
              >
                <label>
                  <input
                    type="checkbox"
                    :checked="resourceIds.includes(type.id)"
                    :disabled="
                      !resourceIds.includes(type.id) && resourceIds.length >= MAX_RESOURCE_FILTERS
                    "
                    @change="toggleResource(type.id, ($event.target as HTMLInputElement).checked)"
                  />
                  <AtlasResourceKey
                    :color="resourceColor(type.id)"
                    :icon-url="resourceIcons.get(type.id)"
                  />
                  <span
                    >{{ type.name }}<small v-if="type.tier > 0"> · T{{ type.tier }}</small></span
                  >
                </label>
                <button
                  type="button"
                  class="atlas-resource-favorite"
                  :aria-label="`Favorite ${type.name}`"
                  :aria-pressed="favoriteResourceIds.has(type.id)"
                  :disabled="favoritesFull && !favoriteResourceIds.has(type.id)"
                  title="Save to Monitor favorites"
                  @click="toggleFavoriteResource(type)"
                >
                  {{ favoriteResourceIds.has(type.id) ? "★" : "☆" }}
                </button>
              </div>
              <p v-if="resourceChoices.more" class="atlas-note">
                Refine your search to see {{ resourceChoices.more }} more.
              </p>
            </div>
            <p v-if="resourcesStatus === 'select'" class="atlas-note">Select resources to show.</p>
            <p v-else-if="resourcesStatus === 'clusters'" class="atlas-note">
              Cluster counts include all selected types. Click a cluster to zoom in.
            </p>
            <p v-else-if="resourcesStatus === 'partial'" class="atlas-note">
              Showing synced regions. Some regions are still loading.
            </p>
            <p v-else-if="resourcesStatus === 'loading'" class="atlas-note" role="status">
              Loading nearby resources…
            </p>
            <p v-else-if="resourcesStatus === 'empty'" class="atlas-note">
              No selected resources found nearby.
            </p>
            <p v-else-if="resourcesStatus === 'truncated'" class="atlas-note">
              Node limit reached. Zoom in or select fewer resource types to see more.
            </p>
            <p
              v-else-if="resourcesStatus === 'sync'"
              class="atlas-note atlas-claim-status"
              role="status"
            >
              Waiting for this region’s first resource sync.
              <button type="button" @click="refreshResources?.()">Retry</button>
            </p>
            <p v-else-if="resourcesStatus === 'error'" class="atlas-note atlas-claim-status">
              Couldn’t load nearby resources.
              <button type="button" @click="refreshResources?.()">Retry</button>
            </p>
          </AtlasLayerGroup>
          <p v-if="atlas && !hasNames" class="atlas-note">No region names in this snapshot.</p>
          <div v-if="showClaims" class="atlas-claim-status" aria-live="polite">
            <p v-if="claimStatus === 'loading'">Loading claims…</p>
            <p v-else-if="claimStatus === 'empty'">No overworld claims are available.</p>
            <p v-else-if="claimStatus === 'error'">
              Couldn’t load claims.
              <button type="button" @click="setClaimsVisible?.(true)">Retry</button>
            </p>
          </div>
        </div>
      </details>
      <aside
        v-if="atlas && showResources && selectedResourceTypes.length"
        class="atlas-resource-legend"
        aria-labelledby="atlas-resource-legend-title"
      >
        <div class="atlas-resource-legend-header">
          <h2 id="atlas-resource-legend-title" class="atlas-eyebrow">Selected resources</h2>
          <button type="button" class="atlas-resource-clear" @click="clearLegendResources">
            Clear all
          </button>
        </div>
        <ul>
          <li v-for="type in selectedResourceTypes" :key="type.id">
            <AtlasResourceKey
              :color="resourceColor(type.id)"
              :icon-url="resourceIcons.get(type.id)"
            />
            <span class="atlas-resource-legend-name"
              >{{ type.name }}<small v-if="type.tier > 0"> · T{{ type.tier }}</small></span
            >
            <AtlasResourceColorPicker
              :name="type.name"
              :color="resourceColor(type.id)"
              :custom="!!resourceColors[type.id]"
              @change="setResourceColor(type.id, $event)"
            />
            <button
              type="button"
              class="atlas-resource-remove"
              :aria-label="`Remove ${type.name} from selected resources`"
              :title="`Remove ${type.name}`"
              @click="removeLegendResource(type.id, $event)"
            >
              ×
            </button>
          </li>
        </ul>
        <p v-if="resourcesStatus === 'truncated'" class="atlas-resource-limit" role="status">
          Node limit reached. Zoom in for more detail.
        </p>
      </aside>
    </div>

    <div v-if="atlas" class="atlas-right-tools">
      <details v-if="currentHasTerrain" name="atlas-menu" class="atlas-menu atlas-monitor-menu">
        <summary class="atlas-menu-button">Monitor</summary>
        <AtlasMonitorPanel
          class="atlas-panel"
          :base="config.public.api.base"
          :preferences="monitorPreferences"
          :rosters="monitorRosters"
          :roster-loading="monitorRosterLoading"
          :roster-error="monitorRosterError"
          :visible-players="visibleTrackedPlayers"
          :stream-status="atlasPlayersStatus"
          :resource-ids="resourceIds"
          :show-resources="showResources"
          @toggle="monitor.toggle"
          @resource="showFavoriteResource"
          @locate="locateTrackedPlayer"
          @player-color="monitor.setPlayerColor"
          @retry="monitor.refreshRosters"
        />
      </details>
      <details name="atlas-menu" class="atlas-menu atlas-region-menu">
        <summary class="atlas-menu-button">Regions</summary>
        <aside class="atlas-panel" aria-label="Regions">
          <div class="atlas-panel-title">
            <p class="atlas-eyebrow">Overworld · Dimension {{ atlas.dimension }}</p>
            <h2>Regions</h2>
            <p>{{ activeCount }} active · {{ atlas.regions.length }} possible</p>
          </div>
          <div
            class="atlas-regions"
            :style="{ '--region-columns': Math.sqrt(atlas.regions.length) }"
          >
            <button
              v-for="region in listedRegions"
              :key="region.id"
              type="button"
              class="atlas-region"
              :aria-label="`Region ${region.id}${region.name ? `: ${region.name}` : ''}, ${region.active ? 'active' : 'inactive'}`"
              :title="region.name"
              :aria-pressed="selected?.id === region.id"
              @click="fitRegion?.(region)"
            >
              <span
                class="atlas-region-number"
                :class="{ 'has-terrain': region.chunks || region.active }"
                >{{ String(region.id).padStart(2, "0") }}</span
              >
            </button>
          </div>
          <p v-if="currentHasTerrain" class="atlas-note">
            {{ terrainRegions }} of {{ atlas.regions.length }} regions have terrain in this
            snapshot.
          </p>
          <p v-else class="atlas-note">
            Highlighted regions are active. Current terrain is not available.
          </p>
        </aside>
      </details>
      <details name="atlas-menu" class="atlas-menu atlas-settings-menu">
        <summary class="atlas-menu-button">Settings</summary>
        <aside class="atlas-panel" aria-label="Map settings">
          <fieldset class="atlas-layer-options">
            <legend class="atlas-eyebrow">Map display</legend>
            <div
              v-for="{ key, label, range } in displaySizeControls"
              :key="key"
              class="atlas-size-control"
            >
              <label :for="`atlas-display-${key}`">{{ label }}</label>
              <div class="atlas-display-size">
                <input
                  :id="`atlas-display-${key}`"
                  v-model.number="displaySizes[key]"
                  type="range"
                  :min="range.min"
                  :max="range.max"
                  :step="range.step"
                  :aria-valuetext="`${displaySizes[key]}%`"
                />
                <span class="atlas-display-value" aria-hidden="true">{{ displaySizes[key] }}%</span>
              </div>
              <button
                class="atlas-display-reset"
                type="button"
                :aria-label="`Reset ${label.toLowerCase()}`"
                :disabled="displaySizes[key] === range.default"
                @click="displaySizes[key] = range.default"
              >
                Reset
              </button>
            </div>
          </fieldset>
          <fieldset class="atlas-layer-options atlas-claim-options">
            <legend class="atlas-eyebrow">Resources</legend>
            <label><input v-model="showResourceIcons" type="checkbox" /> Show resource icons</label>
            <p class="atlas-settings-note">
              Use colored dots when icons are off. Clusters still show counts.
            </p>
          </fieldset>
          <fieldset class="atlas-layer-options atlas-claim-options">
            <legend class="atlas-eyebrow">Players</legend>
            <label><input v-model="showPlayerNames" type="checkbox" /> Show player names</label>
          </fieldset>
          <fieldset class="atlas-layer-options atlas-claim-options">
            <legend class="atlas-eyebrow">Region display</legend>
            <label><input v-model="showGrid" type="checkbox" /> Region grid</label>
            <label><input v-model="showRegionNumbers" type="checkbox" /> Region numbers</label>
            <label>
              <input v-model="showRegionNames" type="checkbox" :disabled="!hasNames" />
              Region names
            </label>
          </fieldset>
          <fieldset class="atlas-layer-options atlas-claim-options">
            <legend class="atlas-eyebrow">Region data</legend>
            <p class="atlas-settings-note">
              Load overlays and live players for these regions. Terrain stays visible.
            </p>
            <div class="atlas-tier-actions">
              <button type="button" @click="setAllRegionsEnabled(true)">Enable all</button>
              <button type="button" @click="setAllRegionsEnabled(false)">Disable all</button>
            </div>
            <fieldset
              v-for="group in settingsRegionGroups"
              :key="group.label"
              class="atlas-settings-region-group"
            >
              <legend class="atlas-eyebrow">{{ group.label }}</legend>
              <div class="atlas-tier-actions">
                <button
                  type="button"
                  :aria-label="`Enable all ${group.label.toLowerCase()}`"
                  @click="setRegionGroupEnabled(group.regions, true)"
                >
                  Enable all
                </button>
                <button
                  type="button"
                  :aria-label="`Disable all ${group.label.toLowerCase()}`"
                  @click="setRegionGroupEnabled(group.regions, false)"
                >
                  Disable all
                </button>
              </div>
              <label v-for="region in group.regions" :key="region.id">
                <input
                  type="checkbox"
                  :checked="!disabledRegionIds.has(region.id)"
                  @change="setRegionEnabled(region.id, ($event.target as HTMLInputElement).checked)"
                />
                Region {{ region.id }}<template v-if="region.name"> · {{ region.name }}</template>
              </label>
            </fieldset>
          </fieldset>
          <p class="atlas-note">Saved in this browser, not in shared links.</p>
        </aside>
      </details>
    </div>
  </div>
</template>

<style scoped>
:global(.atlas-shell) {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background: var(--atlas-ocean-color, #193a4a);
  color: #e0edf0;
}
.atlas-eyebrow {
  font:
    600 0.69rem/1.4 ui-monospace,
    monospace;
  letter-spacing: 0.16em;
  text-transform: uppercase;
}
.atlas-frame {
  position: absolute;
  inset: 0;
}
.atlas-map {
  position: absolute;
  inset: 0;
  cursor: grab;
}
.atlas-map:active {
  cursor: grabbing;
}
.atlas-map:focus-visible {
  outline: 2px solid #f0c78c;
  outline-offset: -3px;
}
.atlas-controls,
.atlas-north,
.atlas-coordinate,
.atlas-status {
  position: absolute;
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  background: #102531;
}
.atlas-controls {
  bottom: 0.85rem;
  right: 0.85rem;
  display: grid;
  overflow: hidden;
}
.atlas-controls button {
  width: 2.5rem;
  height: 2.5rem;
  font:
    500 1.25rem/1 ui-monospace,
    monospace;
  cursor: pointer;
}
.atlas-controls button + button {
  border-top: 1px solid #47616b;
}
.atlas-controls button:hover,
.atlas-menu-button:hover {
  background: #274854;
}
.atlas-controls button:focus-visible,
.atlas-menu-button:focus-visible,
.atlas-layer-options input:focus-visible,
.atlas-region:focus-visible {
  outline: 2px solid #f0c78c;
  outline-offset: -2px;
}
.atlas-north {
  display: grid;
  gap: 0.15rem;
  place-items: center;
  width: 2.5rem;
  padding: 0.55rem 0.35rem;
  top: 0.85rem;
  left: 50%;
  transform: translateX(-50%);
  font:
    600 0.7rem/1 ui-monospace,
    monospace;
  pointer-events: none;
}
.atlas-north span {
  color: #f0c78c;
}
.atlas-coordinate {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem 0.85rem;
  bottom: 0.85rem;
  left: 0.85rem;
  max-width: calc(100% - 4.5rem);
  padding: 0.55rem 0.7rem;
  color: #c3d7de;
  font:
    calc(0.72rem * var(--atlas-info-scale))/1.2 ui-monospace,
    monospace;
  pointer-events: none;
}
.atlas-coordinate span:first-child {
  color: #f0c78c;
}
.atlas-claim-popover {
  position: relative;
  width: min(calc(19rem * var(--atlas-info-scale)), calc(100vw - 2rem));
}
.atlas-claim-popover::after {
  position: absolute;
  bottom: -5px;
  left: calc(50% - 5px);
  width: 10px;
  height: 10px;
  transform: rotate(45deg);
  border-right: 1px solid #846c4e;
  border-bottom: 1px solid #846c4e;
  background: #132b37;
  content: "";
  pointer-events: none;
}
.atlas-claim-popover-below::after {
  top: -5px;
  bottom: auto;
  transform: rotate(225deg);
}
.atlas-claim-card {
  max-height: min(22rem, 52dvh);
  overflow: auto;
  border: 1px solid #846c4e;
  border-radius: 0.55rem;
  background: #132b37;
  padding: 0.85rem;
  box-shadow: 0 6px 18px rgb(0 0 0 / 30%);
}
.atlas-claim-heading {
  display: flex;
  justify-content: space-between;
  gap: 0.5rem;
}
.atlas-claim-heading h2 {
  margin-top: 0.25rem;
  font:
    600 calc(1.25rem * var(--atlas-info-scale))/1.2 Georgia,
    serif;
  overflow-wrap: anywhere;
}
.atlas-claim-heading button {
  align-self: flex-start;
  padding: 0.15rem 0.4rem;
  line-height: 1;
  font-size: 1.25rem;
  cursor: pointer;
}
.atlas-claim-heading button:hover {
  background: #274854;
}
.atlas-claim-heading .atlas-eyebrow,
.atlas-claim-link {
  color: #f0c78c;
}
.atlas-claim-heading .atlas-eyebrow,
.atlas-resource-legend .atlas-eyebrow {
  font-size: calc(0.69rem * var(--atlas-info-scale));
}
.atlas-claim-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 1.5rem;
  margin: 0.75rem 0;
  font-size: calc(0.8rem * var(--atlas-info-scale));
}
.atlas-claim-stats dt,
.atlas-claim-status {
  color: #a9c0c9;
  font-size: 0.75rem;
}
.atlas-claim-stats dd {
  font-weight: 600;
}
.atlas-claim-link,
.atlas-claim-status button {
  font-size: 0.8rem;
  text-decoration: underline;
}
.atlas-claim-link {
  font-size: calc(0.8rem * var(--atlas-info-scale));
}
.atlas-claim-stats dt {
  font-size: calc(0.75rem * var(--atlas-info-scale));
}
.atlas-claim-status button {
  cursor: pointer;
}
.atlas-claim-status:not(:empty) {
  margin-top: 0.5rem;
}
.atlas-menu,
.atlas-layer-stack {
  position: absolute;
  top: 0.85rem;
}
.atlas-layer-stack {
  left: 0.85rem;
  display: grid;
  justify-items: start;
  gap: 0.4rem;
}
.atlas-layer-menu {
  position: static;
}
.atlas-right-tools {
  position: absolute;
  top: 0.85rem;
  right: 0.85rem;
  display: flex;
  gap: 0.4rem;
}
.atlas-right-tools .atlas-menu {
  position: static;
}
.atlas-menu-button {
  width: max-content;
  list-style: none;
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  padding: 0.6rem 0.85rem;
  background: #102531;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
}
.atlas-menu-button::-webkit-details-marker {
  display: none;
}
.atlas-menu-button::after {
  content: "+";
  margin-left: 0.65rem;
  color: #f0c78c;
}
.atlas-menu[open] > .atlas-menu-button::after {
  content: "−";
}
.atlas-region-menu > .atlas-menu-button {
  margin-left: auto;
}
.atlas-panel {
  position: absolute;
  top: calc(100% + 0.4rem);
  width: min(15rem, calc(100vw - 1.7rem));
  max-height: min(32rem, 62dvh);
  overflow: auto;
  overscroll-behavior: contain;
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  background: #132b37;
  padding: 0.8rem;
  box-shadow: 0 6px 18px rgb(0 0 0 / 30%);
}
.atlas-layer-menu .atlas-panel {
  position: static;
  margin-top: 0.4rem;
}
.atlas-layer-stack:has(.atlas-resource-legend) .atlas-panel {
  max-height: min(32rem, 42dvh);
}
.atlas-right-tools .atlas-panel {
  right: 0;
}
.atlas-monitor-menu .atlas-panel {
  width: min(21rem, calc(100vw - 1.7rem));
}
.atlas-settings-note {
  color: #a9c0c9;
  font-size: 0.75rem;
  line-height: 1.4;
}
.atlas-display-size {
  display: flex;
  align-items: center;
  gap: 0.65rem;
}
.atlas-size-control {
  display: grid;
  gap: 0.25rem;
}
.atlas-display-size input {
  flex: 1;
  min-width: 0;
  accent-color: #f0c78c;
}
.atlas-display-value {
  min-width: 4ch;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
.atlas-display-reset {
  justify-self: start;
  color: #f0c78c;
  text-decoration: underline;
  cursor: pointer;
}
.atlas-display-reset:disabled {
  opacity: 0.55;
  cursor: default;
}
.atlas-settings-region-group {
  margin-top: 0.75rem;
}
.atlas-settings-region-group legend {
  color: #c3d7de;
}
.atlas-layer-options legend {
  color: #f0c78c;
  margin-bottom: 0.4rem;
}
.atlas-layer-options label {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.3rem 0;
  font-size: 0.85rem;
  cursor: pointer;
}
.atlas-layer-options input[type="checkbox"] {
  appearance: auto;
  flex: 0 0 1rem;
  width: 1rem;
  height: 1rem;
  accent-color: #f0c78c;
}
.atlas-layer-options label:has(:disabled) {
  opacity: 0.55;
  cursor: default;
}
.atlas-claim-options {
  margin-top: 0.75rem;
  border-top: 1px solid #47616b;
}
.atlas-claim-options legend {
  padding-right: 0.35rem;
}
.atlas-building-key {
  display: inline-grid;
  flex: 0 0 1.25rem;
  height: 1.25rem;
  place-items: center;
  border: 1px solid currentColor;
  border-radius: 50%;
  font:
    700 0.56rem/1 system-ui,
    sans-serif;
}
.atlas-tier-options {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
}
.atlas-player-dot {
  display: inline-block;
  width: 0.55rem;
  height: 0.55rem;
  margin-right: 0.2rem;
  border-radius: 50%;
  background: #79f8ec;
}
.atlas-resource-legend {
  width: fit-content;
  max-width: min(calc(15rem * var(--atlas-info-scale)), calc(100vw - 1.7rem));
  max-height: min(15rem, 32dvh);
  overflow: auto;
  overscroll-behavior: contain;
  border: 1px solid #47616b;
  border-radius: 0.55rem;
  background: #102531;
  padding: 0.5rem 0.65rem;
}
.atlas-resource-legend-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.25rem 0.55rem;
  margin-bottom: 0.3rem;
}
.atlas-resource-legend h2,
.atlas-resource-clear {
  color: #f0c78c;
}
.atlas-resource-clear {
  font-size: 0.72rem;
  text-decoration: underline;
  cursor: pointer;
}
.atlas-resource-remove {
  flex: none;
  width: 1.5rem;
  height: 1.5rem;
  border: 1px solid #47616b;
  border-radius: 0.35rem;
  font-size: 1rem;
  line-height: 1;
  cursor: pointer;
}
.atlas-resource-remove:hover {
  background: #274854;
}
.atlas-resource-legend ul {
  display: grid;
  gap: 0.25rem;
}
.atlas-resource-legend li {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: calc(0.8rem * var(--atlas-info-scale));
  overflow-wrap: anywhere;
}
.atlas-resource-legend-name {
  flex: 1;
}
.atlas-resource-limit {
  margin-top: 0.4rem;
  color: #f0c78c;
  font-size: calc(0.72rem * var(--atlas-info-scale));
  line-height: 1.35;
}
.atlas-resource-options {
  margin-top: 0.65rem;
}
.atlas-resource-choice {
  display: flex;
  align-items: center;
  gap: 0.3rem;
}
.atlas-resource-choice label {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.atlas-resource-favorite {
  flex: none;
  min-width: 1.65rem;
  min-height: 1.65rem;
  color: #f0c78c;
  cursor: pointer;
}
.atlas-resource-favorite:disabled {
  opacity: 0.45;
  cursor: default;
}
.atlas-resource-search {
  display: block;
  width: 100%;
  min-height: 2.75rem;
  margin-bottom: 0.5rem;
  padding: 0.5rem 0.65rem;
  border: 1px solid #47616b;
  border-radius: 0.35rem;
  background: #102531;
  color: inherit;
  font-size: 1rem;
}
.atlas-resource-search::placeholder {
  color: #a9c0c9;
}
.atlas-resource-search:focus-visible {
  outline: 2px solid #f0c78c;
  outline-offset: 1px;
}
.atlas-tier-actions {
  display: flex;
  gap: 0.6rem;
  margin-top: 0.2rem;
  font-size: 0.75rem;
}
.atlas-tier-actions button {
  color: #f0c78c;
  text-decoration: underline;
  cursor: pointer;
}
.atlas-panel-title h2 {
  font:
    600 1.65rem/1.2 Georgia,
    serif;
  margin: 0.3rem 0;
}
.atlas-panel-title p:not(.atlas-eyebrow),
.atlas-note {
  font-size: 0.75rem;
  color: #a9c0c9;
}
.atlas-panel-title .atlas-eyebrow {
  color: #f0c78c;
  letter-spacing: 0.09em;
}
.atlas-regions {
  display: grid;
  grid-template-columns: repeat(var(--region-columns, 5), minmax(0, 1fr));
  gap: 0.25rem;
  margin-top: 0.85rem;
}
.atlas-region {
  display: flex;
  min-width: 0;
  align-items: center;
  justify-content: center;
  padding: 0.2rem 0;
  border: 1px solid transparent;
  border-radius: 0.4rem;
  text-align: left;
  cursor: pointer;
}
.atlas-region:hover {
  background: #203c49;
}
.atlas-region[aria-pressed="true"] {
  border-color: #a78960;
  background: #203c49;
}
.atlas-region-number {
  display: grid;
  flex: 0 0 1.75rem;
  height: 1.75rem;
  place-items: center;
  border: 1px solid #45636f;
  border-radius: 0.3rem;
  color: #bad0d7;
  font:
    600 0.7rem ui-monospace,
    monospace;
}
.atlas-region-number.has-terrain {
  border-color: #758861;
  color: #e0eac9;
  background: #344637;
}
.atlas-note {
  margin-top: 0.9rem;
  line-height: 1.45;
}
.atlas-status {
  top: 50%;
  left: 50%;
  width: max-content;
  max-width: 80%;
  transform: translate(-50%, -50%);
  padding: 1rem;
  text-align: center;
}
@media (max-width: 36rem) {
  .atlas-menu-button {
    padding-inline: 0.55rem;
  }
  .atlas-menu-button::after {
    margin-left: 0.35rem;
  }
  .atlas-north {
    top: 3.7rem;
  }
}
</style>
