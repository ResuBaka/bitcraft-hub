import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Circle as CircleStyle, Fill, Stroke, Style, Text } from "ol/style.js";
import { decodeAtlasPlayerUpdate, mobilePlayerCoordinate } from "./atlasPlayers";
import type { AtlasMarkerScale } from "./atlasMarkerScale";

export type AtlasPlayersStatus = "off" | "connecting" | "live" | "reconnecting";

export function createAtlasPlayersOverlay(
  map: OlMap,
  websocketBase: string,
  signal: AbortSignal,
  setStatus: (status: AtlasPlayersStatus, count: number) => void,
  openSocket = (url: URL) => new WebSocket(url),
  markerScale?: AtlasMarkerScale,
  labelScale?: AtlasMarkerScale,
  onTrackedVisible?: (ids: ReadonlySet<string>) => void,
) {
  const source = new VectorSource<Feature<Point>>({ wrapX: false });
  let tracked: ReadonlyMap<string, string> = new Map();
  const trackedStyles = new Map<string, [Style, Style]>();
  let showAll = false;
  const dot = (radius: number) =>
    new Style({
      image: new CircleStyle({
        radius,
        fill: new Fill({ color: "#79f8ec" }),
        stroke: new Stroke({ color: "#123232", width: 1.5 }),
        declutterMode: "none",
      }),
    });
  const near = dot(5);
  const far = dot(3);
  const layer = new VectorLayer({
    source,
    visible: false,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    declutter: "tracked-player-labels",
    style: (feature, resolution) => {
      const id = String(feature.getId());
      if (!tracked.has(id)) return showAll ? (resolution < 16 ? near : far) : undefined;
      let styles = trackedStyles.get(id);
      if (!styles) {
        const marker = new Style({
          image: new CircleStyle({
            radius: 6,
            fill: new Fill({ color: "#ffd27a" }),
            stroke: new Stroke({ color: "#3b2714", width: 2 }),
            declutterMode: "none",
          }),
          zIndex: 1,
        });
        const label = new Style({
          text: new Text({
            text: tracked.get(id),
            offsetY: -12,
            textBaseline: "bottom",
            font: "600 12px system-ui, sans-serif",
            fill: new Fill({ color: "#fff0d2" }),
            stroke: new Stroke({ color: "#172630", width: 3 }),
          }),
          zIndex: 2,
        });
        styles = [marker, label];
        trackedStyles.set(id, styles);
      }
      markerScale?.apply(styles[0]);
      labelScale?.apply(styles[1]);
      return styles;
    },
  });
  markerScale?.track(layer, () => {
    markerScale.apply(near);
    markerScale.apply(far);
  });
  markerScale?.apply(near);
  markerScale?.apply(far);
  labelScale?.track(layer);
  map.addLayer(layer);
  let enabled = false;
  let regions: string | undefined;
  let socket: WebSocket | undefined;
  let retry: ReturnType<typeof setTimeout> | undefined;
  let retryDelay = 1_000;
  let synced = false;
  let count = 0;
  let visibleTracked = new Set<string>();

  function publishTracked() {
    if (!onTrackedVisible) return;
    const next = new Set([...tracked.keys()].filter((id) => !!source.getFeatureById(id)));
    if (next.size === visibleTracked.size && [...next].every((id) => visibleTracked.has(id)))
      return;
    visibleTracked = next;
    onTrackedVisible(next);
  }

  function clear() {
    synced = false;
    count = 0;
    source.clear(true);
  }

  function retryConnection() {
    if (!enabled || signal.aborted) return;
    setStatus("reconnecting", 0);
    retry = setTimeout(connect, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 15_000);
  }

  function connect() {
    if (!enabled || signal.aborted || socket) return;
    let ws: WebSocket;
    try {
      const url = new URL(
        `${websocketBase.replace(/\/+$/, "")}/websocket`,
        globalThis.location?.href,
      );
      if (url.protocol === "http:") url.protocol = "ws:";
      if (url.protocol === "https:") url.protocol = "wss:";
      url.searchParams.set("stream", "atlasPlayers");
      if (regions) url.searchParams.set("regions", regions);
      ws = openSocket(url);
    } catch {
      retryConnection();
      return;
    }
    socket = ws;
    ws.addEventListener("message", (event) => {
      if (socket !== ws || typeof event.data !== "string") return;
      const update = decodeAtlasPlayerUpdate(event.data);
      if (!update || (!synced && update.type !== "snapshot")) return;
      if (update.type === "snapshot") {
        clear();
        synced = true;
        retryDelay = 1_000;
      } else {
        for (const id of update.remove) {
          const feature = source.getFeatureById(id);
          if (feature && !Array.isArray(feature)) {
            source.removeFeature(feature);
            count--;
          }
        }
      }
      const added: Feature<Point>[] = [];
      for (const [id, x, z] of update.type === "snapshot" ? update.players : update.upsert) {
        const coordinates = mobilePlayerCoordinate(x, z);
        const current = source.getFeatureById(id);
        if (current && !Array.isArray(current)) {
          current.getGeometry()?.setCoordinates(coordinates);
        } else {
          const feature = new Feature(new Point(coordinates));
          feature.setId(id);
          added.push(feature);
        }
      }
      source.addFeatures(added);
      count += added.length;
      publishTracked();
      setStatus("live", count);
    });
    ws.addEventListener("close", () => {
      if (socket !== ws) return;
      socket = undefined;
      clear();
      publishTracked();
      retryConnection();
    });
  }

  function configure(scope = regions, forceOff = false) {
    const next = !forceOff && (showAll || tracked.size > 0) && scope !== "none";
    if ((enabled === next && regions === scope) || (signal.aborted && next)) return;
    regions = scope;
    clearTimeout(retry);
    const previous = socket;
    socket = undefined;
    previous?.close();
    clear();
    publishTracked();
    enabled = next;
    layer.setVisible(next);
    if (next) {
      retryDelay = 1_000;
      setStatus("connecting", 0);
      connect();
    } else {
      setStatus("off", 0);
    }
  }

  function setEnabled(next: boolean, scope?: string) {
    if (showAll !== next) {
      showAll = next;
      layer.changed();
    }
    // An omitted scope means all regions, not the previous scope.
    configure(scope ?? "");
  }

  function setTracking(next: ReadonlyMap<string, string>) {
    tracked = next;
    trackedStyles.clear();
    layer.changed();
    configure();
    publishTracked();
  }

  function trackedPosition(id: string) {
    if (!tracked.has(id)) return;
    const feature = source.getFeatureById(id);
    return !Array.isArray(feature) ? feature?.getGeometry()?.getCoordinates() : undefined;
  }

  signal.addEventListener("abort", () => configure(regions, true), { once: true });
  return { setEnabled, setTracking, trackedPosition };
}
