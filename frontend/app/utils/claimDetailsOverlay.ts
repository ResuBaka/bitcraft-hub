import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import { buffer, getHeight, getWidth } from "ol/extent.js";
import MultiLineString from "ol/geom/MultiLineString.js";
import Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Circle as CircleStyle, Fill, Stroke, Style, Text } from "ol/style.js";
import type { AtlasFetch } from "./atlasBrowserCache";
import type { AtlasMarkerScale } from "./atlasMarkerScale";
import {
  CLAIM_BORDER_MAX_RESOLUTION,
  CLAIM_BUILDING_KINDS,
  CLAIM_BUILDING_MAX_RESOLUTION,
  CLAIM_BUILDING_STYLES,
  claimBorderSegments,
  claimBuildingBounds,
  claimDetailBounds,
  containsClaimBounds,
  type ClaimDetailBounds,
  type ClaimDetailOptions,
  type ClaimDetailsResponse,
  type ClaimBuildingKind,
} from "./claimMapDetails";
import { smallHexCenter } from "./worldMap";

export type ClaimDetailsStatus = "idle" | "zoom" | "loading" | "ready" | "error";
type DetailQuery = ClaimDetailBounds & { borders: boolean; buildings: boolean; regions?: string };
const VIEW_REFRESH_MS = 75;
const REQUEST_INTERVAL_MS = 150;
const FAILED_REQUEST_COOLDOWN_MS = 1_000;

type CachedArea = { bounds: ClaimDetailBounds; expires: number };

function covers(cached: CachedArea | undefined, wanted: ClaimDetailBounds) {
  return !!cached && cached.expires > Date.now() && containsClaimBounds(cached.bounds, wanted);
}

export function createClaimDetailsOverlay(
  map: OlMap,
  signal: AbortSignal,
  settings: () => ClaimDetailOptions & { enabled: boolean },
  setStatus: (status: ClaimDetailsStatus) => void,
  fetchDetails: AtlasFetch<DetailQuery, ClaimDetailsResponse>,
  regionScope: () => string | undefined = () => undefined,
  markerScale?: AtlasMarkerScale,
) {
  let options = settings();
  const borderSource = new VectorSource<Feature<MultiLineString>>({ wrapX: false });
  const buildingSource = new VectorSource<Feature<Point>>({ wrapX: false });
  const borderLayer = new VectorLayer({
    source: borderSource,
    visible: false,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    maxResolution: CLAIM_BORDER_MAX_RESOLUTION,
    style: [
      new Style({ stroke: new Stroke({ color: "#262126c9", width: 3 }) }),
      new Style({ stroke: new Stroke({ color: "#f9da93", width: 1.35 }) }),
    ],
  });
  const styles = Object.fromEntries(
    CLAIM_BUILDING_KINDS.map((kind) => {
      const { badge, color } = CLAIM_BUILDING_STYLES[kind];
      const badgeStyle = (compact: boolean) =>
        new Style({
          image: new CircleStyle({
            radius: compact ? 7 : 9,
            fill: new Fill({ color: "#152b37" }),
            stroke: new Stroke({ color, width: 1.5 }),
          }),
          text: new Text({
            text: badge,
            font: compact ? "700 8px system-ui, sans-serif" : "700 9px system-ui, sans-serif",
            fill: new Fill({ color }),
          }),
        });
      return [kind, { near: badgeStyle(false), regional: badgeStyle(true) }];
    }),
  ) as Record<ClaimBuildingKind, { near: Style; regional: Style }>;
  const buildingLayer = new VectorLayer({
    source: buildingSource,
    visible: false,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    maxResolution: CLAIM_BUILDING_MAX_RESOLUTION,
    declutter: "claim-buildings",
    style: (feature, resolution) => {
      const kind = feature.get("kind") as ClaimBuildingKind;
      if (!options[kind]) return;
      const style = styles[kind][resolution >= 16 ? "regional" : "near"];
      return markerScale?.apply(style) ?? style;
    },
  });
  markerScale?.track(buildingLayer);
  // Keep the primary claim badges above the detail layers.
  const index = map.getLayers().getLength() - 1;
  map.getLayers().insertAt(index, borderLayer);
  map.getLayers().insertAt(index + 1, buildingLayer);

  const view = map.getView();
  let viewTimer: ReturnType<typeof setTimeout> | undefined;
  let requestTimer: ReturnType<typeof setTimeout> | undefined;
  let inflight: AbortController | undefined;
  let wanted: DetailQuery | undefined;
  let nextRequestAt = 0;
  let failedUntil = 0;
  let cachedBorders: CachedArea | undefined;
  let cachedBuildings: CachedArea | undefined;
  let currentRegions: string | undefined;

  function stopRequest() {
    clearTimeout(requestTimer);
    requestTimer = undefined;
    inflight?.abort();
    inflight = undefined;
    wanted = undefined;
  }

  // Throttle rather than debounce: a long drag still gets regular refreshes.
  function scheduleViewRefresh() {
    if (signal.aborted || viewTimer !== undefined || !options.enabled) return;
    viewTimer = setTimeout(() => {
      viewTimer = undefined;
      refresh();
    }, VIEW_REFRESH_MS);
  }
  view.on("change:center", scheduleViewRefresh);
  view.on("change:resolution", scheduleViewRefresh);
  map.on("change:size", scheduleViewRefresh);
  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(viewTimer);
      stopRequest();
      view.un("change:center", scheduleViewRefresh);
      view.un("change:resolution", scheduleViewRefresh);
      map.un("change:size", scheduleViewRefresh);
    },
    { once: true },
  );

  function refresh(filtersChanged = false, retry = false) {
    if (signal.aborted) return;
    const regions = regionScope();
    if (regions !== currentRegions) {
      currentRegions = regions;
      stopRequest();
      cachedBorders = cachedBuildings = undefined;
      failedUntil = 0;
      borderSource.clear(true);
      buildingSource.clear(true);
    }
    options = settings();
    const bordersEnabled = options.border;
    const buildingsEnabled = CLAIM_BUILDING_KINDS.some((kind) => options[kind]);
    const size = map.getSize();
    const resolution = view.getResolution();
    const enabled = options.enabled && (bordersEnabled || buildingsEnabled);
    const showBorders =
      bordersEnabled && resolution !== undefined && resolution < CLAIM_BORDER_MAX_RESOLUTION;
    const showBuildings =
      buildingsEnabled && resolution !== undefined && resolution < CLAIM_BUILDING_MAX_RESOLUTION;
    const extent =
      enabled && regions !== "none" && size && (showBorders || showBuildings)
        ? view.calculateExtent(size)
        : undefined;
    const borderArea = extent && showBorders ? claimDetailBounds(extent) : undefined;
    const buildingArea = extent && showBuildings ? claimBuildingBounds(extent) : undefined;
    borderLayer.setVisible(!!borderArea);
    buildingLayer.setVisible(!!buildingArea);
    if (filtersChanged) buildingLayer.changed();
    if ((!borderArea && !buildingArea) || !extent) {
      stopRequest();
      setStatus(enabled && regions !== "none" ? "zoom" : "idle");
      return;
    }
    const borders = !!borderArea && !covers(cachedBorders, borderArea);
    const buildings = !!buildingArea && !covers(cachedBuildings, buildingArea);
    const visible = borders ? borderArea : buildingArea;
    if ((!borders && !buildings) || !visible) {
      wanted = undefined;
      clearTimeout(requestTimer);
      requestTimer = undefined;
      setStatus("ready");
      return;
    }

    if (retry || filtersChanged) failedUntil = 0;
    if (Date.now() < failedUntil) {
      setStatus("error");
      return;
    }
    // Prefetch past the visible edge to give the next request time to finish
    // during a pan. Stay inside the server's existing bounds limits.
    const margin = Math.min(getWidth(extent), getHeight(extent)) * 0.2;
    const boundsForRequest = borders ? claimDetailBounds : claimBuildingBounds;
    wanted = {
      ...(boundsForRequest(buffer(extent, margin), true) ??
        boundsForRequest(extent, true) ??
        visible),
      borders,
      buildings,
      regions,
    };
    setStatus("loading");
    // Don't cancel a useful request on each camera event. While it runs, the
    // next request follows the latest viewport, not each intermediate position.
    if (inflight || requestTimer !== undefined) return;
    requestTimer = setTimeout(
      async () => {
        requestTimer = undefined;
        const query = wanted;
        if (!query || signal.aborted) return;
        const request = new AbortController();
        inflight = request;
        nextRequestAt = Date.now() + REQUEST_INTERVAL_MS;
        let loaded = false;
        const publish = (data: ClaimDetailsResponse, fresh: boolean) => {
          if (signal.aborted || request.signal.aborted) return;
          // If the user panned back into the cached area, don't replace it with
          // a window they no longer need.
          if (!wanted) return;
          const cache = { bounds: query, expires: fresh ? Date.now() + 15_000 : 0 };
          if (
            query.borders &&
            wanted.borders &&
            (fresh || !cachedBorders || !containsClaimBounds(cachedBorders.bounds, query))
          ) {
            borderSource.clear(true);
            if (data.borders.length) {
              borderSource.addFeature(
                new Feature(new MultiLineString(data.borders.flatMap(claimBorderSegments))),
              );
            }
            cachedBorders = cache;
          }
          if (
            query.buildings &&
            wanted.buildings &&
            (fresh || !cachedBuildings || !containsClaimBounds(cachedBuildings.bounds, query))
          ) {
            buildingSource.clear(true);
            buildingSource.addFeatures(
              data.buildings.map(
                (building) =>
                  new Feature({
                    geometry: new Point(smallHexCenter(building.x, building.z)),
                    kind: building.kind,
                  }),
              ),
            );
            cachedBuildings = cache;
          }
        };
        try {
          const data = await fetchDetails(query, request.signal, (cached) =>
            publish(cached, false),
          );
          if (signal.aborted || request.signal.aborted) return;
          loaded = true;
          publish(data, true);
        } catch {
          if (!request.signal.aborted && !signal.aborted && wanted) {
            failedUntil = Date.now() + FAILED_REQUEST_COOLDOWN_MS;
            setStatus("error");
          }
        } finally {
          if (inflight === request) {
            inflight = undefined;
            if (loaded) refresh();
          }
        }
      },
      Math.max(0, nextRequestAt - Date.now()),
    );
  }

  return refresh;
}
