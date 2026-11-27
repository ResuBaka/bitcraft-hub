import { expect, test } from "bun:test";
import Collection from "ol/Collection.js";
import type OlMap from "ol/Map.js";
import Observable from "ol/Observable.js";
import View from "ol/View.js";
import MultiLineString from "ol/geom/MultiLineString.js";
import type VectorLayer from "ol/layer/Vector.js";
import type Style from "ol/style/Style.js";
import { createAtlasRoadsOverlay } from "../app/utils/atlasRoadsOverlay";

test.each([
  ["truncated", undefined],
  ["overview", 4],
] as const)(
  "paving accepts the API's %s response and zooms in for exact tiles",
  async (limitedStatus, scale) => {
    const view = new View({ center: [700, 700], resolution: 2 });
    const layers = new Collection<VectorLayer>();
    const map = Object.assign(new Observable(), {
      getLayers: () => layers,
      getView: () => view,
      getSize: () => [100, 100],
    }) as unknown as OlMap;
    const controller = new AbortController();
    const loaded = Promise.withResolvers<void>();
    const zoomed = Promise.withResolvers<void>();
    let saturation = 1;
    const requests: { minX: number; centerX: number; centerZ: number; visibleMinX?: number }[] = [];
    const refresh = createAtlasRoadsOverlay(
      map,
      0,
      controller.signal,
      () => true,
      (status) => {
        if (status === limitedStatus) loaded.resolve();
        if (status === "ready") zoomed.resolve();
      },
      async (query) => {
        requests.push(query);
        const first = requests.length === 1;
        return {
          runs: [[120, 121, 140, 7]],
          tiers: { 7: 2 },
          visibleOnly: first,
          truncated: first && limitedStatus === "truncated",
          scale: first ? scale : 1,
        };
      },
      undefined,
      () => saturation,
    );
    try {
      refresh();
      await loaded.promise;
      expect(requests).toHaveLength(1);
      expect(requests[0]!.visibleMinX!).toBeGreaterThan(requests[0]!.minX);
      expect(requests[0]).toMatchObject({ centerX: 121, centerZ: 140, overview: true });
      const features = () => layers.item(0).getSource()!.getFeatures();
      expect(
        features()
          .find((f) => f.get("color"))
          ?.get("overview"),
      ).toBe(scale !== undefined);
      expect(features().some((f) => !!f.getStyleFunction())).toBe(scale === undefined);
      const paved = features().find((feature) => feature.get("color"))!;
      const pavingColor = () =>
        (layers.item(0).getStyleFunction()!(paved, 2) as Style).getFill()?.getColor();
      const standard = pavingColor();
      saturation = 1.25;
      const boosted = pavingColor();
      expect(boosted).not.toEqual(standard);
      saturation = 2;
      expect(pavingColor()).not.toEqual(boosted);
      saturation = 1;
      expect(pavingColor()).toEqual(standard);
      expect(requests).toHaveLength(1);

      view.setResolution(1);
      refresh();
      await zoomed.promise;
      expect(requests).toHaveLength(2);
      expect(requests[1]).toMatchObject({ centerX: 121, centerZ: 140 });
      expect(requests[1]!.visibleMinX).toBeUndefined();
      expect(requests[1]!.minX).toBeGreaterThan(requests[0]!.visibleMinX!);
      expect(
        features()
          .find((f) => f.get("color"))
          ?.get("overview"),
      ).toBe(false);
      const border = features().find((f) => f.getStyleFunction())!;
      const style = border.getStyleFunction()!;
      expect(style(border, 2)).toBeUndefined();
      expect(style(border, 1)).toBeDefined();
      const geometry = (style(border, 1) as Style).getGeometryFunction()(border);
      expect(geometry).toBeInstanceOf(MultiLineString);
      expect((geometry as MultiLineString).getLineStrings()).toHaveLength(11);
    } finally {
      controller.abort();
    }
  },
);
