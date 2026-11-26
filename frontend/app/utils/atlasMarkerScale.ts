import type Style from "ol/style/Style.js";

type Scale = number | [number, number];

function scaled(value: Scale, factor: number): Scale {
  return typeof value === "number" ? value * factor : [value[0] * factor, value[1] * factor];
}

// Apply screen-space scaling to marker styles, never to coordinates, strokes
// on the map itself, or the camera. Keep one base size per cached style.
export function createAtlasMarkerScale(initial: number) {
  let factor = initial;
  const listeners: (() => void)[] = [];
  const bases = new WeakMap<
    Style,
    {
      image: Scale;
      text: Scale;
      offsetX: number;
      offsetY: number;
      padding: number[] | null;
      applied: number;
    }
  >();

  function apply(style: Style) {
    const image = style.getImage();
    const text = style.getText();
    let base = bases.get(style);
    if (!base) {
      base = {
        image: image?.getScale() ?? 1,
        text: text?.getScale() ?? 1,
        offsetX: text?.getOffsetX() ?? 0,
        offsetY: text?.getOffsetY() ?? 0,
        padding: text?.getPadding() ?? null,
        applied: 1,
      };
      bases.set(style, base);
    }
    if (base.applied !== factor) {
      image?.setScale(scaled(base.image, factor));
      if (text) {
        text.setScale(scaled(base.text, factor));
        text.setOffsetX(base.offsetX * factor);
        text.setOffsetY(base.offsetY * factor);
        if (base.padding) text.setPadding(base.padding.map((padding) => padding * factor));
      }
      base.applied = factor;
    }
    return style;
  }

  return {
    apply,
    track(layer: { changed(): void }, update?: () => void) {
      listeners.push(() => {
        update?.();
        layer.changed();
      });
    },
    set(next: number) {
      if (next === factor) return;
      factor = next;
      for (const listener of listeners) listener();
    },
  };
}

export type AtlasMarkerScale = ReturnType<typeof createAtlasMarkerScale>;
