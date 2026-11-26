import { expect, test } from "bun:test";
import Collection from "ol/Collection.js";
import type OlMap from "ol/Map.js";
import Observable from "ol/Observable.js";
import View from "ol/View.js";
import type VectorLayer from "ol/layer/Vector.js";
import type { Circle as CircleStyle, Style } from "ol/style.js";
import { createAtlasMarkerScale } from "../app/utils/atlasMarkerScale";
import { atlasResourceColor, type AtlasResourcesResponse } from "../app/utils/atlasResources";
import { createAtlasResourcesOverlay } from "../app/utils/atlasResourcesOverlay";
import { smallHexCenter } from "../app/utils/worldMap";

test("close-up resource nodes stay visible while a pan fetches the next area", async () => {
  const view = new View({ center: smallHexCenter(100, 100), resolution: 8 });
  const layers = new Collection<VectorLayer>();
  const map = Object.assign(new Observable(), {
    getLayers: () => layers,
    getView: () => view,
    getSize: () => [120, 120],
  }) as unknown as OlMap;
  const controller = new AbortController();
  const clustersReady = Promise.withResolvers<void>();
  const detailReady = Promise.withResolvers<void>();
  const panStarted = Promise.withResolvers<void>();
  const panResponse = Promise.withResolvers<AtlasResourcesResponse>();
  let detailRequests = 0;
  let enabled = true;
  const scale = createAtlasMarkerScale(1);
  const { refresh, setColors } = createAtlasResourcesOverlay(
    map,
    0,
    [{ id: 7, x: 0, z: 0, width: 20, height: 20, chunks: 1 }],
    controller.signal,
    () => ({ enabled, ids: [12] }),
    (status) => {
      if (status === "clusters") clustersReady.resolve();
      if (status === "ready") detailReady.resolve();
    },
    async () => {
      if (++detailRequests === 1) {
        return { nodes: [[100, 100, 12]], truncated: false };
      }
      panStarted.resolve();
      return panResponse.promise;
    },
    async () => ({ clusters: [[1, 1, 10, 12]], partial: false }),
    scale,
  );
  try {
    refresh();
    await clustersReady.promise;
    const [detail, clusters] = layers.getArray();
    expect(clusters!.getVisible()).toBe(true);
    const badge = clusters!.getSource()!.getFeatures()[0]!.getStyle() as Style;
    const fill = (style: Style) => (style.getImage() as CircleStyle).getFill()?.getColor();
    expect(fill(badge)).toBe(atlasResourceColor(12));
    setColors({ 12: "#003366" });
    expect(fill(badge)).toBe("#003366");
    expect(badge.getText()?.getFill()?.getColor()).toBe("#ffffff");
    scale.set(1.5);
    expect(badge.getImage()?.getScale()).toBe(1.5);
    expect(badge.getText()?.getScale()).toBe(1.5);
    view.setResolution(2);
    refresh();
    await detailReady.promise;
    const firstFrame = detail!.getSource();
    expect(firstFrame!.getFeatures()).toHaveLength(1);
    expect(detail!.getVisible()).toBe(true);
    const dot = () => detail!.getStyleFunction()!(firstFrame!.getFeatures()[0]!, 2) as Style;
    expect(fill(dot())).toBe("#003366");
    expect(dot().getImage()?.getScale()).toBe(1.5);
    scale.set(1);
    expect(dot().getImage()?.getScale()).toBe(1);
    setColors({});
    expect(fill(dot())).toBe(atlasResourceColor(12));
    expect(fill(badge)).toBe(atlasResourceColor(12));
    expect(detailRequests).toBe(1);

    view.setCenter(smallHexCenter(180, 100));
    refresh();
    await panStarted.promise;
    expect(detail!.getVisible()).toBe(true);
    expect(clusters!.getVisible()).toBe(false);
    expect(detail!.getSource()).toBe(firstFrame);
    const frameSwapped = Promise.withResolvers<void>();
    detail!.once("change:source", () => frameSwapped.resolve());
    panResponse.resolve({ nodes: [[180, 100, 12]], truncated: false });
    await frameSwapped.promise;
    expect(detail!.getVisible()).toBe(true);
    expect(detail!.getSource()).not.toBe(firstFrame);
    expect(detail!.getSource()!.getFeatures()).toHaveLength(1);

    enabled = false;
    refresh();
    expect(detail!.getVisible()).toBe(false);
    expect(clusters!.getVisible()).toBe(false);
  } finally {
    controller.abort();
  }
});

test("zooming into a capped view refetches the smaller area around the camera", async () => {
  const view = new View({ center: smallHexCenter(152, 138), resolution: 3 });
  const layers = new Collection<VectorLayer>();
  const map = Object.assign(new Observable(), {
    getLayers: () => layers,
    getView: () => view,
    getSize: () => [120, 120],
  }) as unknown as OlMap;
  const controller = new AbortController();
  const capped = Promise.withResolvers<void>();
  const ready = Promise.withResolvers<void>();
  const queries: { minX: number; maxX: number; centerX: number; centerZ: number }[] = [];
  const { refresh } = createAtlasResourcesOverlay(
    map,
    0,
    [{ id: 7, x: 0, z: 0, width: 20, height: 20, chunks: 1 }],
    controller.signal,
    () => ({ enabled: true, ids: [12] }),
    (status) => {
      if (status === "truncated") capped.resolve();
      if (status === "ready") ready.resolve();
    },
    async (query) => {
      queries.push(query);
      return { nodes: [[152, 138, 12]], truncated: queries.length === 1 };
    },
    async () => ({ clusters: [], partial: false }),
  );
  try {
    refresh();
    await capped.promise;
    expect(queries).toHaveLength(1);
    expect(queries[0]).toMatchObject({ centerX: 152, centerZ: 138 });

    view.setResolution(1);
    refresh();
    await ready.promise;
    expect(queries).toHaveLength(2);
    expect(queries[1]).toMatchObject({ centerX: 152, centerZ: 138 });
    expect(queries[1]!.minX).toBeGreaterThan(queries[0]!.minX);
    expect(queries[1]!.maxX).toBeLessThan(queries[0]!.maxX);
  } finally {
    controller.abort();
  }
});
