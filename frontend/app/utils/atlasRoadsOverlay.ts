import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import { buffer, getCenter, getHeight, getWidth } from "ol/extent.js";
import MultiLineString from "ol/geom/MultiLineString.js";
import { vibrantAtlasColor } from "./atlasVibrantColors";
import MultiPolygon from "ol/geom/MultiPolygon.js";
import { fromExtent, type default as Polygon } from "ol/geom/Polygon.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Fill, Stroke, Style } from "ol/style.js";
import type { AtlasFetch } from "./atlasBrowserCache";
import {
  atlasRoadBounds,
  ROAD_MAX_RESOLUTION,
  roadHexBorders,
  roadOverviewRunRing,
  roadRunRing,
  roadTileColor,
  type AtlasRoadsResponse,
} from "./atlasRoads";
import { containsClaimBounds, type ClaimDetailBounds } from "./claimMapDetails";
import { SMALL_HEX_ROW_HEIGHT, SMALL_HEX_WIDTH } from "./worldMap";

export type AtlasRoadsStatus =
  | "off"
  | "zoom"
  | "loading"
  | "ready"
  | "overview"
  | "truncated"
  | "error";
type RoadQuery = ClaimDetailBounds & {
  regions?: string;
  centerX: number;
  centerZ: number;
  overview: boolean;
  visibleMinX?: number;
  visibleMinZ?: number;
  visibleMaxX?: number;
  visibleMaxZ?: number;
};
const REFRESH_MS = 15_000;

export function createAtlasRoadsOverlay(
  map: OlMap,
  layerIndex: number,
  signal: AbortSignal,
  enabled: () => boolean,
  setStatus: (status: AtlasRoadsStatus) => void,
  fetchRoads: AtlasFetch<RoadQuery, AtlasRoadsResponse>,
  regionScope: () => string | undefined = () => undefined,
  saturation: () => number = () => 1,
) {
  const source = new VectorSource<Feature<MultiPolygon | Polygon>>({ wrapX: false });
  const style = (color: string, width: number) =>
    new Style({ fill: new Fill({ color }), stroke: new Stroke({ color, width }) });
  const styles = new Map<string, { near: Style; far: Style; overview: Style }>();
  let styleSaturation = 1;
  const layer = new VectorLayer({
    className: "ol-layer atlas-paving-layer",
    source,
    visible: false,
    maxResolution: ROAD_MAX_RESOLUTION,
    // Reuse render instructions across short pans instead of rebuilding the
    // paving geometry after every few pixels of movement.
    renderBuffer: 128,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    style: (feature, resolution) => {
      const base = feature.get("color") as string;
      const factor = saturation();
      if (factor !== styleSaturation) {
        styleSaturation = factor;
        styles.clear();
      }
      let palette = styles.get(base);
      if (!palette) {
        const color = factor === 1 ? base : vibrantAtlasColor(base, factor);
        palette = {
          near: style(color, 0.65),
          far: style(color, 1.5),
          overview: style(color, 0.5),
        };
        styles.set(base, palette);
      }
      if (feature.get("overview")) return palette.overview;
      return resolution < 2 ? palette.near : palette.far;
    },
  });
  map.getLayers().insertAt(layerIndex, layer);
  const view = map.getView();
  let queued: ReturnType<typeof setTimeout> | undefined;
  let expiryTimer: ReturnType<typeof setTimeout> | undefined;
  let pending: AbortController | undefined;
  let nextRequestAt = 0;
  let currentRegions: string | undefined;
  let cache:
    | {
        bounds: ClaimDetailBounds;
        visible: ClaimDetailBounds;
        centerX: number;
        centerZ: number;
        until: number;
        status: "ready" | "overview" | "truncated";
      }
    | undefined;

  function schedule() {
    if (signal.aborted || !enabled() || queued !== undefined) return;
    queued = setTimeout(
      () => {
        queued = undefined;
        refresh();
      },
      Math.max(75, nextRequestAt - Date.now()),
    );
  }

  function refresh() {
    if (signal.aborted) return;
    const regions = regionScope();
    if (regions !== currentRegions) {
      currentRegions = regions;
      pending?.abort();
      cache = undefined;
      source.clear(true);
    }
    const resolution = view.getResolution();
    const size = map.getSize();
    const active = enabled();
    const extent =
      active &&
      regions !== "none" &&
      size &&
      resolution !== undefined &&
      resolution < ROAD_MAX_RESOLUTION
        ? view.calculateExtent(size)
        : undefined;
    const visible = extent && atlasRoadBounds(extent);
    layer.setVisible(!!visible);
    clearTimeout(expiryTimer);
    if (!extent || !visible) {
      pending?.abort();
      setStatus(!active ? "off" : regions === "none" ? "ready" : "zoom");
      return;
    }
    const [x, z] = getCenter(extent);
    const centerX = Math.max(
      visible.minX,
      Math.min(visible.maxX, Math.round(x! / SMALL_HEX_WIDTH)),
    );
    const centerZ = Math.max(
      visible.minZ,
      Math.min(visible.maxZ, Math.round(z! / SMALL_HEX_ROW_HEIGHT)),
    );
    if (
      cache &&
      cache.until > Date.now() &&
      containsClaimBounds(cache.bounds, visible) &&
      // A capped result was picked for its own center and zoom. A closer view
      // needs a fresh result even if it lies inside the previous bounds.
      (cache.status === "ready" ||
        (cache.centerX === centerX &&
          cache.centerZ === centerZ &&
          containsClaimBounds(cache.visible, visible) &&
          containsClaimBounds(visible, cache.visible)))
    ) {
      setStatus(cache.status);
      expiryTimer = setTimeout(refresh, cache.until - Date.now());
      return;
    }
    if (pending) return;
    if (nextRequestAt > Date.now()) {
      schedule();
      return;
    }
    const margin = Math.min(getHeight(extent), getWidth(extent)) * 0.2;
    const padded =
      cache && cache.status !== "ready" ? undefined : atlasRoadBounds(buffer(extent, margin), true);
    const overscan = padded && !containsClaimBounds(visible, padded) ? padded : undefined;
    const request = new AbortController();
    pending = request;
    nextRequestAt = Date.now() + 180;
    if (!cache) setStatus("loading");
    void (async () => {
      let bounds = overscan ?? visible;
      const query = overscan
        ? {
            ...overscan,
            regions,
            centerX,
            centerZ,
            overview: true,
            visibleMinX: visible.minX,
            visibleMinZ: visible.minZ,
            visibleMaxX: visible.maxX,
            visibleMaxZ: visible.maxZ,
          }
        : { ...visible, regions, centerX, centerZ, overview: true };
      const publish = (data: AtlasRoadsResponse, covered: ClaimDetailBounds, fresh: boolean) => {
        if (request.signal.aborted || signal.aborted) return;
        const scale = data.scale ?? 1;
        const overview = scale > 1;
        const polygonsByColor = new Map<string, number[][][][]>();
        for (const run of data.runs) {
          const tileTypeId = run[3];
          const color = roadTileColor(
            tileTypeId,
            tileTypeId === undefined ? undefined : data.tiers?.[tileTypeId],
          );
          const polygon = [overview ? roadOverviewRunRing(run, scale) : roadRunRing(run)];
          const polygons = polygonsByColor.get(color);
          if (polygons) polygons.push(polygon);
          else polygonsByColor.set(color, [polygon]);
        }
        source.clear(true);
        source.addFeatures(
          Array.from(
            polygonsByColor,
            ([color, polygons]) =>
              new Feature({ geometry: new MultiPolygon(polygons), color, overview }),
          ),
        );
        if (!overview && polygonsByColor.size) {
          // Use the fill extent for spatial indexing, but only expand exact
          // runs into edges if the current zoom actually needs them.
          const borders = new Feature(fromExtent(source.getExtent()));
          let geometry: MultiLineString | undefined;
          const borderStyle = new Style({
            geometry: () => (geometry ??= new MultiLineString(roadHexBorders(data.runs))),
            stroke: new Stroke({ color: "rgba(0, 0, 0, 0.28)", width: 0.65 }),
            zIndex: 1,
          });
          borders.setStyle((_feature, resolution) => (resolution < 2 ? borderStyle : undefined));
          source.addFeature(borders);
        }
        cache = {
          bounds: covered,
          visible,
          centerX,
          centerZ,
          until: fresh ? Date.now() + REFRESH_MS : 0,
          status: data.truncated ? "truncated" : overview ? "overview" : "ready",
        };
        setStatus(cache.status);
      };
      let data = await fetchRoads(query, request.signal, (cached) => {
        if (cache && containsClaimBounds(cache.bounds, visible)) return;
        if (cached.truncated && overscan && !cached.visibleOnly) return;
        publish(cached, cached.visibleOnly ? visible : bounds, false);
      });
      if (data.visibleOnly) {
        bounds = visible;
      } else if (data.truncated && overscan && !request.signal.aborted) {
        // Compatibility with API builds that don't handle the fallback.
        bounds = visible;
        data = await fetchRoads(
          { ...bounds, regions, centerX, centerZ, overview: true },
          request.signal,
        );
      }
      publish(data, bounds, true);
    })()
      .catch(() => {
        if (request.signal.aborted || signal.aborted) return;
        setStatus("error");
        nextRequestAt = Date.now() + 1_000;
        expiryTimer = setTimeout(refresh, REFRESH_MS);
      })
      .finally(() => {
        pending = undefined;
        if (request.signal.aborted || (cache && cache.until > Date.now())) schedule();
      });
  }

  view.on("change:center", schedule);
  view.on("change:resolution", schedule);
  map.on("change:size", schedule);
  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(queued);
      clearTimeout(expiryTimer);
      pending?.abort();
      view.un("change:center", schedule);
      view.un("change:resolution", schedule);
      map.un("change:size", schedule);
    },
    { once: true },
  );
  return refresh;
}
