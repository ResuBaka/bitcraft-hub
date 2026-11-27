import Feature from "ol/Feature.js";
import type OlMap from "ol/Map.js";
import Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import VectorSource from "ol/source/Vector.js";
import { Circle as CircleStyle, Fill, Stroke, Style, Text } from "ol/style.js";
import { decodeAtlasPlayerUpdate, mobilePlayerCoordinate } from "./atlasPlayers";
import type { AtlasMarkerScale } from "./atlasMarkerScale";
import { DEFAULT_TRACKED_PLAYER_COLOR } from "./atlasMonitor";

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
  fetchNames?: (ids: string[], signal: AbortSignal) => Promise<Record<string, string>>,
) {
  const source = new VectorSource<Feature<Point>>({ wrapX: false });
  let tracked: ReadonlyMap<string, string> = new Map();
  let colors: Readonly<Record<string, string>> = {};
  const customDots = new Map<string, Style>();
  const labels = new Map<string, { name: string; highlighted: boolean; style: Style }>();
  const names = new Map<string, string>();
  const pendingNames = new Set<string>();
  let nameBatch: Set<string> | undefined;
  let nameTimer: ReturnType<typeof setTimeout> | undefined;
  let nextNameAttempt = 0;
  let showAll = false;
  let showNames = false;
  const dot = (radius: number, trackedColor?: string) =>
    new Style({
      image: new CircleStyle({
        radius,
        fill: new Fill({ color: trackedColor ?? "#79f8ec" }),
        stroke: new Stroke({
          color: trackedColor ? "#3b2714" : "#123232",
          width: trackedColor ? 2 : 1.5,
        }),
        declutterMode: "none",
      }),
      zIndex: trackedColor ? 1 : 0,
    });
  const near = dot(5);
  const far = dot(3);
  const highlightedDot = dot(6, DEFAULT_TRACKED_PLAYER_COLOR);
  function trackedDot(id: string) {
    const color = colors[id];
    if (!color) return highlightedDot;
    let marker = customDots.get(color);
    if (!marker) {
      marker = dot(6, color);
      customDots.set(color, marker);
    }
    return markerScale?.apply(marker) ?? marker;
  }
  const layer = new VectorLayer({
    source,
    visible: false,
    updateWhileAnimating: true,
    updateWhileInteracting: true,
    declutter: "tracked-player-labels",
    style: (feature, resolution) => {
      const id = String(feature.getId());
      const highlighted = tracked.has(id);
      if (!highlighted && !showAll) return;
      const marker = highlighted ? trackedDot(id) : resolution < 16 ? near : far;
      if (!showNames) return marker;
      const name = names.get(id) || tracked.get(id);
      if (!name) return marker;
      let label = labels.get(id);
      if (!label || label.name !== name || label.highlighted !== highlighted) {
        const style = new Style({
          text: new Text({
            text: name,
            textBaseline: "bottom",
            font: "600 12px system-ui, sans-serif",
            fill: new Fill({ color: highlighted ? "#fff0d2" : "#d5fffc" }),
            stroke: new Stroke({ color: "#172630", width: 3 }),
          }),
          zIndex: highlighted ? 3 : 2,
        });
        label = { name, highlighted, style };
        labels.set(id, label);
      }
      labelScale?.apply(label.style);
      const image = marker.getImage() as CircleStyle;
      label.style.getText()?.setOffsetY(-(image.getRadius() + 1) * image.getScaleArray()[1] - 4);
      return [marker, label.style];
    },
  });
  const scaleMarkers = () => {
    for (const marker of [near, far, highlightedDot]) markerScale?.apply(marker);
  };
  markerScale?.track(layer, scaleMarkers);
  scaleMarkers();
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

  function scheduleNames() {
    if (
      !showNames ||
      !fetchNames ||
      signal.aborted ||
      nameBatch ||
      nameTimer !== undefined ||
      !pendingNames.size
    )
      return;
    nameTimer = setTimeout(
      async () => {
        nameTimer = undefined;
        const ids: string[] = [];
        for (const id of pendingNames) {
          pendingNames.delete(id);
          if ((showAll || tracked.has(id)) && source.getFeatureById(id) && !tracked.get(id))
            ids.push(id);
          if (ids.length === 128) break;
        }
        if (!ids.length) return;
        nameBatch = new Set(ids);
        try {
          const loaded = await fetchNames(ids, signal);
          if (!signal.aborted) {
            for (const id of ids) names.set(id, typeof loaded[id] === "string" ? loaded[id] : "");
            layer.changed();
          }
        } catch {
          if (!signal.aborted) {
            for (const id of ids) pendingNames.add(id);
            nextNameAttempt = Date.now() + 30_000;
          }
        } finally {
          nameBatch = undefined;
          scheduleNames();
        }
      },
      Math.max(80, nextNameAttempt - Date.now()),
    );
  }

  function queueName(id: string) {
    if (
      !showNames ||
      !fetchNames ||
      !(showAll || tracked.has(id)) ||
      tracked.get(id) ||
      names.has(id) ||
      nameBatch?.has(id)
    )
      return;
    pendingNames.add(id);
  }

  function queueVisibleNames() {
    if (!showNames || !fetchNames) return;
    source.forEachFeature((feature) => queueName(String(feature.getId())));
    scheduleNames();
  }

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
    labels.clear();
    pendingNames.clear();
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
          labels.delete(id);
          pendingNames.delete(id);
          const feature = source.getFeatureById(id);
          if (feature && !Array.isArray(feature)) {
            source.removeFeature(feature);
            count--;
          }
        }
      }
      const added: Feature<Point>[] = [];
      for (const [id, x, z] of update.type === "snapshot" ? update.players : update.upsert) {
        queueName(id);
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
      scheduleNames();
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
    if (next) queueVisibleNames();
  }

  function setTracking(next: ReadonlyMap<string, string>) {
    tracked = next;
    labels.clear();
    layer.changed();
    configure();
    publishTracked();
    queueVisibleNames();
  }

  function trackedPosition(id: string) {
    if (!tracked.has(id)) return;
    const feature = source.getFeatureById(id);
    return !Array.isArray(feature) ? feature?.getGeometry()?.getCoordinates() : undefined;
  }

  function setColors(next: Readonly<Record<string, string>>) {
    colors = next;
    customDots.clear();
    layer.changed();
  }

  function setShowNames(next: boolean) {
    if (next === showNames) return;
    showNames = next;
    if (next) {
      queueVisibleNames();
    } else {
      clearTimeout(nameTimer);
      nameTimer = undefined;
      pendingNames.clear();
    }
    layer.changed();
  }

  signal.addEventListener(
    "abort",
    () => {
      clearTimeout(nameTimer);
      configure(regions, true);
    },
    { once: true },
  );
  return { setEnabled, setTracking, trackedPosition, setColors, setShowNames };
}
