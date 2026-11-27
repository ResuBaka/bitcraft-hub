import { expect, test } from "bun:test";
import type OlMap from "ol/Map.js";
import type Point from "ol/geom/Point.js";
import VectorLayer from "ol/layer/Vector.js";
import type Style from "ol/style/Style.js";
import type CircleStyle from "ol/style/Circle.js";
import { createAtlasMarkerScale } from "../app/utils/atlasMarkerScale";
import { decodeAtlasPlayerUpdate, mobilePlayerCoordinate } from "../app/utils/atlasPlayers";
import { createAtlasPlayersOverlay } from "../app/utils/atlasPlayersOverlay";
import { SMALL_HEX_WIDTH } from "../app/utils/worldMap";

test("preserves exact player IDs and validates live position frames", () => {
  const id = "18446744073709551615";
  const delta = { type: "delta", upsert: [[id, 1050, 4000]], remove: ["2"] };
  expect(decodeAtlasPlayerUpdate(JSON.stringify(delta))).toEqual(delta);
  expect(decodeAtlasPlayerUpdate('{"type":"snapshot","players":[[1,2,3]]}')).toBeUndefined();
});

test("maps fractional game coordinates into the atlas projection", () => {
  expect(mobilePlayerCoordinate(1000, 2000)).toEqual([SMALL_HEX_WIDTH, 10]);
  const [x, z] = mobilePlayerCoordinate(1550, 4001);
  expect(x).toBeCloseTo(1.5505 * SMALL_HEX_WIDTH);
  expect(z).toBeCloseTo(20.005);
});

test("live dots move, disappear, and clear when the socket closes", () => {
  const layers: VectorLayer[] = [];
  const map = { addLayer: (layer: VectorLayer) => layers.push(layer) } as unknown as OlMap;
  const controller = new AbortController();
  const socket = Object.assign(new EventTarget(), { close: () => {} });
  const status: Array<[string, number]> = [];
  let opened: URL | undefined;
  const { setEnabled } = createAtlasPlayersOverlay(
    map,
    "https://example.test",
    controller.signal,
    (phase, count) => status.push([phase, count]),
    (url) => {
      opened = url;
      return socket as unknown as WebSocket;
    },
  );
  const message = (data: unknown) =>
    socket.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(data) }));
  try {
    expect(opened).toBeUndefined();
    setEnabled(true);
    expect(opened?.href).toBe("wss://example.test/websocket?stream=atlasPlayers");
    const source = layers[0]!.getSource()!;
    message({
      type: "snapshot",
      players: [
        ["1", 1000, 2000],
        ["2", 3000, 4000],
      ],
    });
    const player = source.getFeatures().find((feature) => feature.getId() === "1")!;
    expect((player.getGeometry() as Point).getCoordinates()).toEqual(
      mobilePlayerCoordinate(1000, 2000),
    );
    message({ type: "delta", upsert: [["1", 1100, 2200]], remove: ["2"] });
    expect((player.getGeometry() as Point).getCoordinates()).toEqual(
      mobilePlayerCoordinate(1100, 2200),
    );
    expect(source.getFeatures()).toEqual([player]);
    expect(status.at(-1)).toEqual(["live", 1]);
    socket.dispatchEvent(new Event("close"));
    expect(source.getFeatures()).toEqual([]);
    expect(status.at(-1)).toEqual(["reconnecting", 0]);
  } finally {
    controller.abort();
  }
});

test("tracking uses the regional live stream without displaying untracked players", () => {
  const layers: VectorLayer[] = [];
  const map = { addLayer: (layer: VectorLayer) => layers.push(layer) } as unknown as OlMap;
  const controller = new AbortController();
  const socket = Object.assign(new EventTarget(), { close: () => {} });
  const locations: string[][] = [];
  const opened: URL[] = [];
  const overlay = createAtlasPlayersOverlay(
    map,
    "https://example.test",
    controller.signal,
    () => {},
    (url) => {
      opened.push(url);
      return socket as unknown as WebSocket;
    },
    undefined,
    undefined,
    (ids) => locations.push([...ids]),
  );
  const message = (data: unknown) =>
    socket.dispatchEvent(new MessageEvent("message", { data: JSON.stringify(data) }));
  try {
    overlay.setEnabled(false, "7,8");
    overlay.setTracking(
      new Map([
        ["1", "Alice"],
        ["3", "Cora"],
      ]),
    );
    expect(opened[0]?.searchParams.get("regions")).toBe("7,8");
    message({
      type: "snapshot",
      players: [
        ["1", 1000, 2000],
        ["2", 3000, 4000],
      ],
    });
    expect(locations.at(-1)).toEqual(["1"]);
    expect(overlay.trackedPosition("1")).toEqual(mobilePlayerCoordinate(1000, 2000));
    expect(overlay.trackedPosition("2")).toBeUndefined();
    const layer = layers[0]!;
    const other = layer.getSource()!.getFeatureById("2")!;
    expect(layer.getStyleFunction()!(other, 2)).toBeUndefined();
    overlay.setEnabled(true, "7,8");
    expect(layer.getStyleFunction()!(other, 2)).toBeDefined();
    overlay.setTracking(new Map([["2", "Bea"]]));
    expect(locations.at(-1)).toEqual(["2"]);
    expect(opened).toHaveLength(1);
    overlay.setEnabled(false, "none");
    expect(locations.at(-1)).toEqual([]);
    expect(layer.getVisible()).toBe(false);
  } finally {
    controller.abort();
  }
});

test("tracked and other visible online players show names above their markers", async () => {
  const layers: VectorLayer[] = [];
  const map = { addLayer: (layer: VectorLayer) => layers.push(layer) } as unknown as OlMap;
  const controller = new AbortController();
  const socket = Object.assign(new EventTarget(), { close: () => {} });
  const requested = Promise.withResolvers<string[]>();
  const scale = createAtlasMarkerScale(1);
  const overlay = createAtlasPlayersOverlay(
    map,
    "https://example.test",
    controller.signal,
    () => {},
    () => socket as unknown as WebSocket,
    scale,
    undefined,
    undefined,
    async (ids) => {
      requested.resolve(ids);
      return { "2": "Bea", "3": "Cora" };
    },
  );
  try {
    overlay.setEnabled(true);
    overlay.setTracking(
      new Map([
        ["1", "Alice"],
        ["3", ""],
      ]),
    );
    socket.dispatchEvent(
      new MessageEvent("message", {
        data: JSON.stringify({
          type: "snapshot",
          players: [
            ["1", 1000, 2000],
            ["2", 1100, 2200],
            ["3", 1200, 4000],
          ],
        }),
      }),
    );
    const layer = layers[0]!;
    const player = layer.getSource()!.getFeatureById("1")!;
    expect(Array.isArray(layer.getStyleFunction()!(player, 2))).toBe(false);
    overlay.setShowNames(true);
    const label = (id: string) =>
      (
        layer.getStyleFunction()!(layer.getSource()!.getFeatureById(id)!, 2) as Style[]
      )[1]?.getText();
    expect(label("1")?.getText()).toBe("Alice");
    const nameFrame = Promise.withResolvers<void>();
    layer.once("change", () => nameFrame.resolve());
    expect(await requested.promise).toEqual(["2", "3"]);
    await nameFrame.promise;
    expect(label("2")?.getText()).toBe("Bea");
    expect(label("3")?.getText()).toBe("Cora");
    const markerColor = (id: string) => {
      const feature = layer.getSource()!.getFeatureById(id)!;
      const styles = layer.getStyleFunction()!(feature, 2) as Style[];
      return (styles[0]!.getImage() as CircleStyle).getFill()?.getColor();
    };
    overlay.setColors({ "1": "#123456", "2": "#654321" });
    expect(markerColor("1")).toBe("#123456");
    expect(markerColor("2")).toBe("#79f8ec");
    expect(markerColor("3")).toBe("#ffd27a");
    overlay.setColors({});
    expect(markerColor("1")).toBe("#ffd27a");
    const normal = label("1")!.getOffsetY();
    expect(normal).toBeLessThan(0);
    scale.set(1.75);
    expect(label("1")!.getOffsetY()).toBeLessThan(normal);
    overlay.setShowNames(false);
    expect(Array.isArray(layer.getStyleFunction()!(player, 2))).toBe(false);
  } finally {
    controller.abort();
  }
});
