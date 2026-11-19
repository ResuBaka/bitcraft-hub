import {
  accountOverviewBlocks,
  isAccountOverviewBlockId,
  type AccountOverviewBlockId,
} from "./accountOverviewBlocks";

export type GridPosition = {
  x: number;
  y: number;
  w: number;
  h: number;
};
export type SharedBlock = GridPosition & { id: AccountOverviewBlockId };
export type SharedLayout = {
  format: "bitcraft-hub-player-layout";
  version: 1;
  blocks: SharedBlock[];
};

export function parsePlayerLayout(text: string): SharedLayout | null {
  if (text.length > 10000) return null;
  try {
    return validatePlayerLayout(JSON.parse(text));
  } catch {
    return null;
  }
}

export function validatePlayerLayout(value: unknown): SharedLayout | null {
  if (!value || typeof value !== "object") return null;
  const data = value as Record<string, unknown>;
  if (
    data.format !== "bitcraft-hub-player-layout" ||
    data.version !== 1 ||
    !Array.isArray(data.blocks) ||
    data.blocks.length > accountOverviewBlocks.length
  )
    return null;
  const blocks: SharedBlock[] = [];
  for (const raw of data.blocks) {
    if (!raw || typeof raw !== "object") return null;
    const { id, x, y, w, h } = raw as Record<string, unknown>;
    if (
      typeof id !== "string" ||
      !isAccountOverviewBlockId(id) ||
      blocks.some((block) => block.id === id)
    )
      return null;
    if (![x, y, w, h].every((n) => typeof n === "number" && Number.isSafeInteger(n))) return null;
    const block = { id, x: x as number, y: y as number, w: w as number, h: h as number };
    if (
      block.x < 0 ||
      block.y < 0 ||
      block.y > 1000 ||
      block.w < 3 ||
      block.w > 12 ||
      block.h < 2 ||
      block.h > 100 ||
      block.x + block.w > 12
    )
      return null;
    if (
      blocks.some(
        (other) =>
          block.x < other.x + other.w &&
          other.x < block.x + block.w &&
          block.y < other.y + other.h &&
          other.y < block.y + block.h,
      )
    )
      return null;
    blocks.push(block);
  }
  return { format: "bitcraft-hub-player-layout", version: 1, blocks };
}
