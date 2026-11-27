import { asArray } from "ol/color.js";

export function vibrantAtlasColor(color: string, saturation: number): string {
  const [red = 0, green = 0, blue = 0, alpha = 1] = asArray(color);
  // Match the Canvas saturate() filter used for raster tiles.
  const luminance = 0.213 * red + 0.715 * green + 0.072 * blue;
  const channel = (value: number) =>
    Math.round(Math.max(0, Math.min(255, luminance + (value - luminance) * saturation)));
  return `rgba(${channel(red)}, ${channel(green)}, ${channel(blue)}, ${alpha})`;
}
