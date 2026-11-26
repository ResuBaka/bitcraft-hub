import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import { buffer, getHeight, getWidth } from "ol/extent.js";
import Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Fill, Stroke, Style, Text } from "ol/style.js";
import type { AtlasMarkerScale } from "./atlasMarkerScale";
import { claimDetailBounds, containsClaimBounds, type ClaimDetailBounds } from "./claimMapDetails";
import {
  MAP_BUILDING_MAX_RESOLUTION,
  MAP_BUILDING_STYLES,
  type MapBuilding,
  type MapBuildingGroup,
  type MapBuildingsResponse,
} from "./mapBuildings";
import { smallHexCenter } from "./worldMap";

type Query = ClaimDetailBounds & { groups: string; regions?: string };
export type MapBuildingsStatus = "idle" | "zoom" | "loading" | "ready" | "truncated" | "error";

export function createMapBuildingsOverlay(
  map: OlMap,
  signal: AbortSignal,
  enabledGroups: () => readonly MapBuildingGroup[],
  setStatus: (status: MapBuildingsStatus) => void,
  fetchBuildings: (query: Query, signal: AbortSignal) => Promise<MapBuildingsResponse>,
  regionScope: () => string | undefined = () => undefined,
  markerScale?: AtlasMarkerScale,
) {
  const source = new VectorSource<Feature<Point>>({ wrapX: false });
  const styles = new Map<string, Style>();
  let groups = new Set<MapBuildingGroup>();
  let currentGroupKey = "";
  let currentRegions: string | undefined;
  const layer = new VectorLayer({
    source,
    visible: false,
    maxResolution: MAP_BUILDING_MAX_RESOLUTION,
    declutter: "claim-buildings",
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    renderOrder: (a, b) => (b.get("building").tier ?? 0) - (a.get("building").tier ?? 0),
    style: (feature) => {
      const { group, tier } = feature.get("building") as MapBuilding;
      if (!groups.has(group)) return;
      const { badge, color } = MAP_BUILDING_STYLES[group];
      const key = tier === undefined ? badge : `${badge} T${tier}`;
      let style = styles.get(key);
      if (!style) {
        style = new Style({
          text: new Text({
            text: key,
            font: "700 9px system-ui, sans-serif",
            padding: [2, 3, 2, 3],
            fill: new Fill({ color }),
            backgroundFill: new Fill({ color: "#152b37" }),
            backgroundStroke: new Stroke({ color, width: 1 }),
          }),
        });
        styles.set(key, style);
      }
      return markerScale?.apply(style) ?? style;
    },
  });
  markerScale?.track(layer);
  // Prefer tier badges to overlapping service markers at close zoom.
  map.getLayers().insertAt(map.getLayers().getLength() - 1, layer);
  const view = map.getView();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let pending: AbortController | undefined;
  let nextRequestAt = 0;
  let cached: { query: Query; expires: number; truncated: boolean } | undefined;

  function schedule() {
    if (!signal.aborted && timer === undefined) {
      timer = setTimeout(
        () => {
          timer = undefined;
          refresh();
        },
        Math.max(75, nextRequestAt - Date.now()),
      );
    }
  }

  function refresh() {
    if (signal.aborted) return;
    const regions = regionScope();
    if (regions !== currentRegions) {
      currentRegions = regions;
      pending?.abort();
      cached = undefined;
      source.clear(true);
    }
    const enabled = enabledGroups();
    const groupKey = enabled.join(",");
    if (groupKey !== currentGroupKey) {
      currentGroupKey = groupKey;
      groups = new Set(enabled);
      layer.changed();
    }
    const resolution = view.getResolution();
    const size = map.getSize();
    const extent =
      regions !== "none" &&
      size &&
      resolution !== undefined &&
      resolution < MAP_BUILDING_MAX_RESOLUTION
        ? view.calculateExtent(size)
        : undefined;
    const visible = extent && claimDetailBounds(extent);
    layer.setVisible(!!groupKey && !!visible);
    if (!groupKey || !visible || !extent) {
      pending?.abort();
      setStatus(groupKey && regions !== "none" ? "zoom" : "idle");
      return;
    }
    if (
      cached &&
      cached.query.groups === groupKey &&
      cached.expires > Date.now() &&
      containsClaimBounds(cached.query, visible)
    ) {
      setStatus(cached.truncated ? "truncated" : "ready");
      return;
    }
    if (pending) return;
    if (Date.now() < nextRequestAt) {
      schedule();
      return;
    }
    const margin = Math.min(getWidth(extent), getHeight(extent)) * 0.2;
    const query = {
      ...(claimDetailBounds(buffer(extent, margin), true) ??
        claimDetailBounds(extent, true) ??
        visible),
      groups: groupKey,
      regions,
    };
    const request = new AbortController();
    pending = request;
    nextRequestAt = Date.now() + 150;
    setStatus("loading");
    void fetchBuildings(query, request.signal)
      .then(({ markers, truncated }) => {
        if (signal.aborted || request.signal.aborted || enabledGroups().join(",") !== groupKey)
          return;
        source.clear(true);
        source.addFeatures(
          markers.map(
            (building) =>
              new Feature({
                geometry: new Point(smallHexCenter(building.x, building.z)),
                building,
              }),
          ),
        );
        cached = { query, expires: Date.now() + 15_000, truncated };
        setStatus(truncated ? "truncated" : "ready");
      })
      .catch(() => {
        if (signal.aborted || request.signal.aborted) return;
        nextRequestAt = Date.now() + 1_000;
        if (enabledGroups().join(",") === groupKey) setStatus("error");
      })
      .finally(() => {
        pending = undefined;
        if (
          request.signal.aborted ||
          enabledGroups().join(",") !== groupKey ||
          (cached?.query === query && !signal.aborted)
        )
          schedule();
      });
  }

  view.on("change:center", schedule);
  view.on("change:resolution", schedule);
  map.on("change:size", schedule);
  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(timer);
      pending?.abort();
      view.un("change:center", schedule);
      view.un("change:resolution", schedule);
      map.un("change:size", schedule);
    },
    { once: true },
  );
  return refresh;
}
