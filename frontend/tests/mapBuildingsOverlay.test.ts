import { expect, test } from "bun:test";
import Collection from "ol/Collection.js";
import type OlMap from "ol/Map.js";
import Observable from "ol/Observable.js";
import View from "ol/View.js";
import VectorLayer from "ol/layer/Vector.js";
import type Style from "ol/style/Style.js";
import type { MapBuildingGroup } from "../app/utils/mapBuildings";
import { createMapBuildingsOverlay } from "../app/utils/mapBuildingsOverlay";

test("building overlay labels known tiers, filters groups, and hides at wide zoom", async () => {
  const view = new View({ center: [100, 100], resolution: 1 });
  const layers = new Collection([new VectorLayer()]);
  const map = Object.assign(new Observable(), {
    getLayers: () => layers,
    getView: () => view,
    getSize: () => [100, 100],
  }) as unknown as OlMap;
  const controller = new AbortController();
  let groups: MapBuildingGroup[] = ["crafting", "storage"];
  let groupQuery: string | undefined;
  const refresh = createMapBuildingsOverlay(
    map,
    controller.signal,
    () => groups,
    () => {},
    async (query) => {
      groupQuery = query.groups;
      return {
        markers: [
          { x: 16, z: 19, group: "crafting", tier: 5 },
          { x: 18, z: 22, group: "storage" },
        ],
        truncated: false,
      };
    },
  );
  try {
    refresh();
    await Promise.resolve();
    expect(groupQuery).toBe("crafting,storage");
    const layer = layers.item(0);
    const features = layer.getSource()!.getFeatures();
    const style = layer.getStyleFunction()!;
    const workshop = features.find((feature) => feature.get("building").group === "crafting")!;
    const storage = features.find((feature) => feature.get("building").group === "storage")!;
    expect((style(workshop, 1) as Style).getText()?.getText()).toBe("C T5");
    expect((style(storage, 1) as Style).getText()?.getText()).toBe("S");
    groups = ["crafting"];
    refresh();
    expect(style(storage, 1)).toBeUndefined();
    view.setResolution(8);
    refresh();
    expect(layer.getVisible()).toBe(false);
  } finally {
    controller.abort();
  }
});
