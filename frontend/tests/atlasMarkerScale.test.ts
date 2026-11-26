import { expect, test } from "bun:test";
import { Circle, Style, Text } from "ol/style.js";
import { createAtlasMarkerScale } from "../app/utils/atlasMarkerScale";

test("map marker scale preserves native image sizes and label spacing across adjustments", () => {
  const image = new Circle({ radius: 8, scale: 0.5 });
  const text = new Text({ scale: [1, 2], offsetY: -12, padding: [2, 4, 2, 4] });
  const style = new Style({ image, text });
  const scale = createAtlasMarkerScale(1.5);
  let renders = 0;
  scale.track({ changed: () => renders++ });
  scale.apply(style);
  expect(image.getScale()).toBe(0.75);
  expect(text.getScale()).toEqual([1.5, 3]);
  expect(text.getOffsetY()).toBe(-18);
  expect(text.getPadding()).toEqual([3, 6, 3, 6]);
  scale.set(1);
  scale.apply(style);
  scale.set(1);
  expect(renders).toBe(1);
  expect(image.getScale()).toBe(0.5);
  expect(text.getScale()).toEqual([1, 2]);
  expect(text.getOffsetY()).toBe(-12);
  expect(text.getPadding()).toEqual([2, 4, 2, 4]);
});
