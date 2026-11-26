import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import { buffer, getCenter, getHeight, getWidth } from "ol/extent.js";
import MultiPoint from "ol/geom/MultiPoint.js";
import Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import type { Pixel } from "ol/pixel.js";
import VectorSource from "ol/source/Vector.js";
import { Circle as CircleStyle, Fill, Icon, Stroke, Style, Text } from "ol/style.js";
import type { AtlasFetch } from "./atlasBrowserCache";
import type { AtlasMarkerScale } from "./atlasMarkerScale";
import {
  atlasResourceColor,
  atlasResourceIconSize,
  atlasResourceRegions,
  atlasResourceTextColor,
  placeResourceClusters,
  resourceClusterBounds,
  resourceClusterBoundsFit,
  resourceClusterExtent,
  resourceClusterLevel,
  resourceClusterWorldBounds,
  RESOURCE_MAX_RESOLUTION,
  type AtlasResourceClustersResponse,
  type AtlasResourceColors,
  type AtlasResourcesResponse,
} from "./atlasResources";
import {
  claimBuildingBounds,
  claimDetailBounds,
  containsClaimBounds,
  type ClaimDetailBounds,
} from "./claimMapDetails";
import {
  SMALL_HEX_ROW_HEIGHT,
  SMALL_HEX_WIDTH,
  smallHexCenter,
  type WorldMapRegion,
} from "./worldMap";

export type ResourcesStatus =
  | "off"
  | "select"
  | "loading"
  | "sync"
  | "ready"
  | "clusters"
  | "partial"
  | "empty"
  | "truncated"
  | "error";
type Query = ClaimDetailBounds & { ids: string; regions: string };
type DetailQuery = Query & { centerX: number; centerZ: number };
type ClusterQuery = Query & { level: number };
type Plan =
  | { mode: "detail"; query: DetailQuery; visible: ClaimDetailBounds }
  | { mode: "clusters"; query: ClusterQuery; visible: ClaimDetailBounds };
type Cached = { plan: Plan; expires: number; status: ResourcesStatus };
type ClusterExtent = ReturnType<typeof resourceClusterExtent>;
const compact = new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 });
function shadedResourceIcon(image: HTMLImageElement, scale: number) {
  // Lift dark sprites off the terrain without a hard outline or backplate.
  // Paint at 2× resolution to keep the shadow consistent at every zoom.
  const density = 2;
  const width = image.naturalWidth * scale * density;
  const height = image.naturalHeight * scale * density;
  const canvas = document.createElement("canvas");
  canvas.width = Math.ceil(width) + 4 * density;
  canvas.height = Math.ceil(height) + 4 * density;
  const ctx = canvas.getContext("2d");
  if (!ctx) return new Icon({ img: image, scale });

  ctx.shadowColor = "rgba(12, 19, 25, 0.65)";
  ctx.shadowBlur = 1.5 * density;
  ctx.shadowOffsetY = 0.65 * density;
  if (typeof ctx.filter === "string") ctx.filter = "brightness(1.3)";
  ctx.drawImage(image, (canvas.width - width) / 2, (canvas.height - height) / 2, width, height);
  return new Icon({ img: canvas, scale: 1 / density });
}

function intersects(a: ClaimDetailBounds, b: ClaimDetailBounds) {
  return a.minX <= b.maxX && a.maxX >= b.minX && a.minZ <= b.maxZ && a.maxZ >= b.minZ;
}

export function createAtlasResourcesOverlay(
  map: OlMap,
  layerIndex: number,
  regions: WorldMapRegion[],
  signal: AbortSignal,
  settings: () => {
    enabled: boolean;
    ids: readonly number[];
    disabledRegions?: ReadonlySet<number>;
  },
  setStatus: (status: ResourcesStatus) => void,
  fetchResources: AtlasFetch<DetailQuery, AtlasResourcesResponse>,
  fetchClusters: AtlasFetch<ClusterQuery, AtlasResourceClustersResponse>,
  markerScale?: AtlasMarkerScale,
) {
  const clusterSource = new VectorSource<Feature<Point>>({ wrapX: false });
  type NodeStyles = {
    url?: string;
    dot: Style;
    icon?: { image: HTMLImageElement; size: number; styles: Map<number, Style> };
  };
  const styles = new Map<number, NodeStyles>();
  let icons: ReadonlyMap<number, string> = new Map();
  let colors: AtlasResourceColors = {};
  const dotStyle = (color: string) =>
    new Style({
      image: new CircleStyle({
        radius: 3,
        fill: new Fill({ color }),
        stroke: new Stroke({ color: "#282131", width: 1.1 }),
      }),
    });
  let selected = new Set<number>();
  const layer = new VectorLayer({
    source: new VectorSource<Feature<MultiPoint>>({ wrapX: false }),
    visible: false,
    maxResolution: RESOURCE_MAX_RESOLUTION,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    style: (feature, resolution) => {
      const id = feature.get("resourceId") as number;
      if (!selected.has(id)) return;
      const url = icons.get(id);
      let entry = styles.get(id);
      if (!entry || entry.url !== url) {
        const next: NodeStyles = {
          url,
          dot: dotStyle(atlasResourceColor(id, colors)),
        };
        styles.set(id, next);
        entry = next;
        if (url) {
          const image = new Image();
          image.onload = () => {
            const size = Math.max(image.naturalWidth, image.naturalHeight);
            if (!size || signal.aborted || styles.get(id) !== next || icons.get(id) !== url) return;
            next.icon = { image, size, styles: new Map() };
            layer.changed();
          };
          // Keep the colored dot while loading, or if the sprite is unavailable.
          image.src = url;
        }
      }
      if (!entry.icon) return markerScale?.apply(entry.dot) ?? entry.dot;
      const { image, size, styles: iconStyles } = entry.icon;
      const pixels = atlasResourceIconSize(resolution);
      let style = iconStyles.get(pixels);
      if (!style) {
        style = new Style({
          image: shadedResourceIcon(image, pixels / size),
        });
        iconStyles.set(pixels, style);
      }
      return markerScale?.apply(style) ?? style;
    },
  });
  const clusterLayer = new VectorLayer({
    source: clusterSource,
    visible: false,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    declutter: "resource-clusters",
    renderOrder: (a, b) => Number(b.get("count")) - Number(a.get("count")),
  });
  markerScale?.track(layer);
  markerScale?.track(clusterLayer, () => {
    clusterSource.forEachFeature((feature) => {
      markerScale.apply(feature.getStyle() as Style);
    });
  });
  map.getLayers().insertAt(layerIndex, layer);
  map.getLayers().insertAt(layerIndex + 1, clusterLayer);
  const view = map.getView();
  let currentIds = "";
  let currentDisabledRegions: ReadonlySet<number> | undefined;
  let enabledRegions = regions;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let syncTimer: ReturnType<typeof setTimeout> | undefined;
  let pending: { controller: AbortController; mode: Plan["mode"] } | undefined;
  let nextRequestAt = 0;
  let detailCache: Cached | undefined;
  let clusterCache: Cached | undefined;

  function retryWhenSynced() {
    if (syncTimer !== undefined) return;
    syncTimer = setTimeout(() => {
      syncTimer = undefined;
      refresh();
    }, 16_000);
  }

  function schedule() {
    if (!settings().enabled || signal.aborted || timer !== undefined) return;
    timer = setTimeout(
      () => {
        timer = undefined;
        refresh();
      },
      Math.max(75, nextRequestAt - Date.now()),
    );
  }

  function requestPlan(extent: number[], resolution: number, ids: string): Plan | undefined {
    const margin = Math.min(getWidth(extent), getHeight(extent)) * 0.15;
    const buffered = buffer(extent, margin);
    const detail = resolution < RESOURCE_MAX_RESOLUTION ? claimDetailBounds(extent) : undefined;
    if (detail) {
      // Once a view is capped, don't spend the next request's budget on
      // offscreen nodes or round its area up to whole chunks.
      const limited = detailCache?.status === "truncated" && detailCache.plan.query.ids === ids;
      const bounds = limited ? detail : (claimDetailBounds(buffered, true) ?? detail);
      const [x, z] = getCenter(extent);
      const centerX = Math.max(
        detail.minX,
        Math.min(detail.maxX, Math.round(x! / SMALL_HEX_WIDTH)),
      );
      const centerZ = Math.max(
        detail.minZ,
        Math.min(detail.maxZ, Math.round(z! / SMALL_HEX_ROW_HEIGHT)),
      );
      const names = atlasResourceRegions(bounds, enabledRegions);
      if (names.length && names.length <= 4)
        return {
          mode: "detail",
          query: { ...bounds, centerX, centerZ, ids, regions: names.join(",") },
          visible: detail,
        };
      const visibleRegions = atlasResourceRegions(detail, enabledRegions);
      if (visibleRegions.length && visibleRegions.length <= 4)
        return {
          mode: "detail",
          query: { ...detail, centerX, centerZ, ids, regions: visibleRegions.join(",") },
          visible: detail,
        };
    }
    const world = claimBuildingBounds(extent);
    if (!world) return;
    const level = resourceClusterLevel(world, resolution);
    const visible = resourceClusterBounds(world, level);
    const overscan = resourceClusterBounds(claimBuildingBounds(buffered) ?? world, level);
    const bounds = resourceClusterBoundsFit(overscan) ? overscan : visible;
    const names = atlasResourceRegions(resourceClusterWorldBounds(bounds, level), enabledRegions);
    if (names.length) {
      return {
        mode: "clusters",
        query: { ...bounds, ids, regions: names.join(","), level },
        visible,
      };
    }
  }

  function currentPlan(ids: string) {
    const resolution = view.getResolution();
    const size = map.getSize();
    if (ids && size && resolution !== undefined)
      return requestPlan(view.calculateExtent(size), resolution, ids);
  }

  function compatible(a: Plan, b: Plan) {
    return (
      a.query.ids === b.query.ids &&
      a.mode === b.mode &&
      (a.mode !== "clusters" || (b.mode === "clusters" && a.query.level === b.query.level))
    );
  }

  function covers(cache: Cached | undefined, plan: Plan) {
    return (
      !!cache &&
      cache.expires > Date.now() &&
      compatible(cache.plan, plan) &&
      containsClaimBounds(cache.plan.query, plan.visible) &&
      // A capped frame is only valid for its original viewport. A closer
      // zoom must fetch again even if it fits inside the previous bounds.
      (cache.status !== "truncated" ||
        (cache.plan.mode === "detail" &&
          plan.mode === "detail" &&
          cache.plan.query.centerX === plan.query.centerX &&
          cache.plan.query.centerZ === plan.query.centerZ &&
          containsClaimBounds(cache.plan.visible, plan.visible) &&
          containsClaimBounds(plan.visible, cache.plan.visible)))
    );
  }

  function refresh() {
    if (signal.aborted) return;
    const { enabled, ids, disabledRegions } = settings();
    if (disabledRegions !== currentDisabledRegions) {
      currentDisabledRegions = disabledRegions;
      enabledRegions = regions.filter(({ id }) => !disabledRegions?.has(id));
      pending?.controller.abort();
      detailCache = clusterCache = undefined;
      layer.getSource()?.clear(true);
      clusterSource.clear(true);
      layer.setVisible(false);
      clusterLayer.setVisible(false);
    }
    const idKey = enabled ? ids.join(",") : "";
    if (idKey !== currentIds) {
      currentIds = idKey;
      selected = new Set(enabled ? ids : []);
      layer.changed();
      pending?.controller.abort();
      layer.setVisible(false);
      clusterLayer.setVisible(false);
    }
    const plan = currentPlan(idKey);
    if (!plan) {
      // While panning outside a covered region the same frame can stay
      // mounted; its features will naturally pan out of view.
      clearTimeout(syncTimer);
      syncTimer = undefined;
      setStatus(!enabled ? "off" : !idKey ? "select" : "empty");
      return;
    }
    const cache = plan.mode === "detail" ? detailCache : clusterCache;
    const available = covers(cache, plan);
    const hasClusters = clusterCache?.plan.query.ids === idKey;
    const hasDetails = detailCache?.plan.query.ids === idKey;
    // Expiring or leaving the cached bounds triggers a fetch, not a switch
    // back to cluster badges after individual nodes are already on screen.
    const showDetails =
      plan.mode === "detail" &&
      hasDetails &&
      (layer.getVisible() || intersects(detailCache.plan.query, plan.visible));
    layer.setVisible(showDetails);
    clusterLayer.setVisible(!showDetails && hasClusters);
    if (pending?.mode && pending.mode !== plan.mode) pending.controller.abort();
    if (available && cache) {
      setStatus(cache.status);
      if (cache.status === "partial") retryWhenSynced();
      return;
    }
    if (pending) return;
    if (nextRequestAt > Date.now()) {
      schedule();
      return;
    }
    const controller = new AbortController();
    pending = { controller, mode: plan.mode };
    nextRequestAt = Date.now() + 250;
    if (!hasClusters && !hasDetails) setStatus("loading");
    let completed = false;
    const canPublish = () => {
      if (controller.signal.aborted || signal.aborted) return false;
      const current = currentPlan(currentIds);
      if (!current || !compatible(plan, current) || !intersects(plan.query, current.visible))
        return false;
      // A response started before reversing direction shouldn't replace
      // a frame that still covers more of the current view.
      return (
        !cache ||
        !compatible(cache.plan, current) ||
        !containsClaimBounds(cache.plan.query, current.visible) ||
        containsClaimBounds(plan.query, current.visible)
      );
    };
    const shouldPreview =
      !cache ||
      !compatible(cache.plan, plan) ||
      !containsClaimBounds(cache.plan.query, plan.visible);
    void (async () => {
      if (plan.mode === "detail") {
        const publish = (data: AtlasResourcesResponse, fresh: boolean) => {
          if (!canPublish()) return;
          const coordinates = new Map<number, number[][]>();
          for (const [x, z, id] of data.nodes) {
            if (!selected.has(id)) continue;
            const point = smallHexCenter(x, z);
            const points = coordinates.get(id);
            if (points) points.push(point);
            else coordinates.set(id, [point]);
          }
          // Swap in the populated frame instead of clearing the displayed one.
          layer.setSource(
            new VectorSource({
              wrapX: false,
              features: Array.from(
                coordinates,
                ([resourceId, points]) =>
                  new Feature({ geometry: new MultiPoint(points), resourceId }),
              ),
            }),
          );
          detailCache = {
            plan,
            expires: fresh ? Date.now() + 15_000 : 0,
            status: data.truncated ? "truncated" : data.nodes.length ? "ready" : "empty",
          };
          if (!fresh) {
            setStatus(detailCache.status);
            refresh();
          }
        };
        const data = await fetchResources(
          plan.query,
          controller.signal,
          shouldPreview ? (cached) => publish(cached, false) : undefined,
        );
        completed = !controller.signal.aborted && !signal.aborted;
        publish(data, true);
      } else {
        const publish = (data: AtlasResourceClustersResponse, fresh: boolean) => {
          if (!canPublish()) return;
          const { level } = plan.query;
          clusterSource.clear(true);
          clusterSource.addFeatures(
            placeResourceClusters(data.clusters, level).map(({ center, extent, count, id }) => {
              const color = atlasResourceColor(id, colors);
              const feature = new Feature({
                geometry: new Point(center),
                extent,
                count,
                resourceId: id,
              });
              const style = new Style({
                image: new CircleStyle({
                  radius: Math.min(22, 11 + Math.log10(count) * 2),
                  fill: new Fill({ color }),
                  stroke: new Stroke({ color: "#282131", width: 2 }),
                  // Keep the label and its badge together during decluttering.
                }),
                text: new Text({
                  text: compact.format(count),
                  font: "700 11px system-ui, sans-serif",
                  fill: new Fill({ color: atlasResourceTextColor(color) }),
                }),
              });
              feature.setStyle(markerScale?.apply(style) ?? style);
              return feature;
            }),
          );
          clusterCache = {
            plan,
            expires: fresh ? Date.now() + 15_000 : 0,
            status: data.partial ? "partial" : data.clusters.length ? "clusters" : "empty",
          };
          if (!fresh) {
            setStatus(clusterCache.status);
            refresh();
          }
        };
        const data = await fetchClusters(
          plan.query,
          controller.signal,
          shouldPreview ? (cached) => publish(cached, false) : undefined,
        );
        completed = !controller.signal.aborted && !signal.aborted;
        publish(data, true);
      }
    })()
      .catch((error: unknown) => {
        if (controller.signal.aborted || signal.aborted) return;
        nextRequestAt = Date.now() + 1_000;
        const waitingForSync =
          error instanceof Error && "statusCode" in error && error.statusCode === 503;
        setStatus(waitingForSync ? "sync" : "error");
        if (waitingForSync) retryWhenSynced();
      })
      .finally(() => {
        pending = undefined;
        if (completed) refresh();
        else if (controller.signal.aborted) schedule();
      });
  }

  function clusterAtPixel(pixel: Pixel): ClusterExtent | undefined {
    if (!clusterLayer.getVisible()) return;
    return map.forEachFeatureAtPixel(pixel, (feature) => feature.get("extent") as ClusterExtent, {
      layerFilter: (candidate) => candidate === clusterLayer,
      hitTolerance: 2,
    });
  }

  function zoomToCluster(extent: ClusterExtent) {
    view.cancelAnimations();
    view.fit(extent, { padding: [36, 36, 36, 36], duration: 275 });
  }

  function setIcons(next: ReadonlyMap<number, string>) {
    icons = next;
    layer.changed();
  }

  function setColors(next: AtlasResourceColors) {
    const previous = colors;
    colors = next;
    for (const [id, entry] of styles) {
      if (next[id] !== previous[id]) entry.dot = dotStyle(atlasResourceColor(id, colors));
    }
    clusterSource.forEachFeature((feature) => {
      const id = feature.get("resourceId") as number;
      if (next[id] === previous[id]) return;
      const color = atlasResourceColor(id, colors);
      const style = feature.getStyle() as Style;
      (style.getImage() as CircleStyle).setFill(new Fill({ color }));
      style.getText()?.setFill(new Fill({ color: atlasResourceTextColor(color) }));
    });
    layer.changed();
    clusterLayer.changed();
  }

  view.on("change:center", schedule);
  view.on("change:resolution", schedule);
  map.on("change:size", schedule);
  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(timer);
      clearTimeout(syncTimer);
      pending?.controller.abort();
      view.un("change:center", schedule);
      view.un("change:resolution", schedule);
      map.un("change:size", schedule);
    },
    { once: true },
  );
  return { refresh, clusterAtPixel, zoomToCluster, setIcons, setColors };
}
