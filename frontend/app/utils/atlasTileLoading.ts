import type OlMap from "ol/Map.js";
import type TileQueue from "ol/TileQueue.js";

export const ATLAS_MAX_TILES_LOADING = 12;
const ZOOM_TILES_PER_FRAME = 6;
const ZOOM_FRAME_BUDGET_MS = 12;

export function loadAtlasZoomTiles(
  queue: Pick<TileQueue, "isEmpty" | "getTilesLoading" | "reprioritize" | "loadMoreTiles">,
  frameTime: number,
  now: number,
) {
  if (
    now - frameTime >= ZOOM_FRAME_BUDGET_MS ||
    queue.isEmpty() ||
    queue.getTilesLoading() >= ATLAS_MAX_TILES_LOADING
  )
    return;
  queue.reprioritize();
  queue.loadMoreTiles(ATLAS_MAX_TILES_LOADING, ZOOM_TILES_PER_FRAME);
}

export function enableAtlasZoomTileLoading(map: OlMap, terrainVisible: () => boolean) {
  const view = map.getView();
  let previousResolution = 0;
  map.on("postrender", ({ frameState }) => {
    if (!frameState) return;
    const { resolution, nextResolution } = frameState.viewState;
    const zoomingIn =
      (view.getAnimating() && nextResolution !== undefined && nextResolution < resolution) ||
      (view.getInteracting() && resolution < previousResolution);
    previousResolution = resolution;

    // OpenLayers normally starts only two tiles per animation frame (or
    // none on a busy frame). Reuse its populated, prioritized queue rather
    // than starting a second set of HTTP requests. Keep this frame-state
    // integration here so other layers don't depend on the queue internals.
    if (zoomingIn && terrainVisible() && frameState.tileQueue)
      loadAtlasZoomTiles(frameState.tileQueue, frameState.time, Date.now());
  });
}
