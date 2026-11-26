import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import { buffer, getHeight, getWidth } from "ol/extent.js";
import MultiPolygon from "ol/geom/MultiPolygon.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Fill, Stroke, Style } from "ol/style.js";
import type { WorldMapAtlas } from "./worldMap";
import {
  containsTerrainHexBounds,
  TERRAIN_HEX_MAX_RESOLUTION,
  terrainHexBounds,
  terrainHexCenter,
  terrainHexRing,
  terrainPixelSampler,
  type TerrainRasterSample,
} from "./terrainHexes";

const TILE_CACHE_SIZE = 16;
const VIEW_REFRESH_MS = 65;

export function createTerrainHexOverlay(
  map: OlMap,
  data: WorldMapAtlas,
  base: string,
  signal: AbortSignal,
  visible: () => boolean,
) {
  const pixelSampler = terrainPixelSampler(data);
  if (!pixelSampler) return;
  const sample = pixelSampler;
  const tiles = new Map<number, Promise<Uint8ClampedArray | undefined>>();
  const source = new VectorSource<Feature<MultiPolygon>>({ wrapX: false });
  const layer = new VectorLayer({
    source,
    visible: false,
    maxResolution: TERRAIN_HEX_MAX_RESOLUTION,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    style: (feature) => feature.get("hexStyle") as Style,
  });
  // Existing raster tiles remain underneath as a fallback while colors load.
  map.getLayers().insertAt(1, layer);
  const view = map.getView();
  let pending = false;
  let covered: ReturnType<typeof terrainHexBounds>;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function readPixels(tile: TerrainRasterSample) {
    let result = tiles.get(tile.key);
    if (!result) {
      result = (async () => {
        const response = await fetch(
          `${base}${data.version}/${tile.zoom}/${tile.x}/${tile.y}.webp`,
          { signal },
        );
        if (!response.ok) return;
        const bitmap = await createImageBitmap(await response.blob());
        try {
          if (signal.aborted || bitmap.width !== data.tileSize || bitmap.height !== data.tileSize)
            return;
          const canvas = document.createElement("canvas");
          canvas.width = canvas.height = data.tileSize;
          const context = canvas.getContext("2d", { willReadFrequently: true });
          if (!context) return;
          context.drawImage(bitmap, 0, 0);
          return context.getImageData(0, 0, data.tileSize, data.tileSize).data;
        } finally {
          bitmap.close();
        }
      })().catch(() => undefined);
    }
    // Touch the LRU slot, retaining one read per tile across adjacent viewports.
    tiles.delete(tile.key);
    tiles.set(tile.key, result);
    if (tiles.size > TILE_CACHE_SIZE) {
      const oldest = tiles.keys().next().value;
      if (oldest !== undefined) tiles.delete(oldest);
    }
    return result;
  }

  function queue() {
    if (signal.aborted || timer !== undefined) return;
    timer = setTimeout(() => {
      timer = undefined;
      refresh();
    }, VIEW_REFRESH_MS);
  }

  function extentForLayer() {
    const size = map.getSize();
    const resolution = view.getResolution();
    if (!visible() || !size || resolution === undefined || resolution >= TERRAIN_HEX_MAX_RESOLUTION)
      return;
    return view.calculateExtent(size);
  }

  function refresh() {
    if (signal.aborted) return;
    const extent = extentForLayer();
    const needed = extent && terrainHexBounds(extent);
    layer.setVisible(!!needed);
    if (!extent || !needed || pending || (covered && containsTerrainHexBounds(covered, needed)))
      return;
    const margin = Math.min(getWidth(extent), getHeight(extent)) * 0.2;
    const requested = terrainHexBounds(buffer(extent, margin)) ?? needed;
    const cells: Array<{ cx: number; cz: number; key: number; pixel: number }> = [];
    const reads = new Map<number, Promise<Uint8ClampedArray | undefined>>();
    for (let z = requested.minZ; z <= requested.maxZ; z++) {
      for (let x = requested.minX; x <= requested.maxX; x++) {
        const [cx, cz] = terrainHexCenter(x, z);
        const tile = sample(cx, cz);
        if (!tile) continue;
        if (!reads.has(tile.key)) reads.set(tile.key, readPixels(tile));
        cells.push({ cx, cz, key: tile.key, pixel: tile.pixel });
      }
    }

    pending = true;
    void Promise.all([...reads].map(async ([key, pixels]) => [key, await pixels] as const))
      .then((results) => {
        if (signal.aborted) return;
        const pixelsByTile = new Map(results);
        const ringsByColor = new Map<number, number[][][]>();
        for (const { cx, cz, key, pixel } of cells) {
          const rgba = pixelsByTile.get(key);
          // Legacy caches keep missing terrain transparent.
          if (!rgba || (rgba[pixel + 3] ?? 0) < 250) continue;
          const color = (rgba[pixel]! << 16) | (rgba[pixel + 1]! << 8) | rgba[pixel + 2]!;
          const ring = terrainHexRing(cx, cz);
          const rings = ringsByColor.get(color);
          if (rings) rings.push(ring);
          else ringsByColor.set(color, [ring]);
        }
        const features = [...ringsByColor].map(([color, rings]) => {
          const fill = `#${color.toString(16).padStart(6, "0")}`;
          return new Feature({
            geometry: new MultiPolygon(rings.map((ring) => [ring])),
            hexStyle: new Style({
              fill: new Fill({ color: fill }),
              // Cover subpixel antialiasing gaps between adjoining fills.
              stroke: new Stroke({ color: fill, width: 0.6 }),
            }),
          });
        });
        source.clear(true);
        source.addFeatures(features);
        covered = requested;
      })
      .finally(() => {
        pending = false;
        if (!signal.aborted) queue();
      });
  }

  view.on("change:center", queue);
  view.on("change:resolution", queue);
  map.on("change:size", queue);
  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(timer);
      tiles.clear();
      view.un("change:center", queue);
      view.un("change:resolution", queue);
      map.un("change:size", queue);
    },
    { once: true },
  );
  return refresh;
}
