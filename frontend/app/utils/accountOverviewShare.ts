import {
  validatePlayerLayout,
  type GridPosition,
  type SharedLayout,
} from "./accountOverviewLayout";

export type TrackedPlayer = { id: string; name: string };
export type SharedDashboard = TrackedPlayer & {
  position: GridPosition;
  layout: SharedLayout;
  activeClaims: string[];
  passiveClaims: string[];
};

const prefix = "#setup=";
const maxLength = 16000;
const maxPlayers = 20;
const entityId = /^[1-9]\d{0,19}$/;

function validId(value: unknown): value is string {
  return (
    typeof value === "string" && entityId.test(value) && BigInt(value) <= 18446744073709551615n
  );
}

function validClaims(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length <= 50 &&
    new Set(value).size === value.length &&
    value.every(
      (id) =>
        typeof id === "string" &&
        (id === "unknown" ||
          id === "unclaimed" ||
          (id.startsWith("claim:") && validId(id.slice(6)))),
    )
  );
}

function parseSetup(value: unknown): SharedDashboard[] | null {
  if (!Array.isArray(value) || value.length !== 2 || value[0] !== 1) return null;
  const entries: unknown = value[1];
  if (!Array.isArray(entries) || entries.length < 1 || entries.length > maxPlayers) return null;
  const dashboards: SharedDashboard[] = [];
  for (const entry of entries) {
    if (!Array.isArray(entry) || entry.length !== 6) return null;
    const [id, name, box, blocks, activeClaims, passiveClaims] = entry;
    if (
      !validId(id) ||
      dashboards.some((player) => player.id === id) ||
      typeof name !== "string" ||
      !name.trim() ||
      name.length > 128 ||
      !Array.isArray(box) ||
      box.length !== 4 ||
      !Array.isArray(blocks) ||
      blocks.length > 4 ||
      !validClaims(activeClaims) ||
      !validClaims(passiveClaims)
    )
      return null;
    const [x, y, w, h] = box;
    if (
      ![x, y, w, h].every(Number.isSafeInteger) ||
      x < 0 ||
      y < 0 ||
      y > 2000 ||
      w < 3 ||
      w > 12 ||
      x + w > 12 ||
      h < 2 ||
      h > 100
    )
      return null;
    const layout = validatePlayerLayout({
      format: "bitcraft-hub-player-layout",
      version: 1,
      blocks: blocks.map((block: unknown) => {
        if (!Array.isArray(block) || block.length !== 5) return null;
        const [id, x, y, w, h] = block;
        return { id, x, y, w, h };
      }),
    });
    if (!layout) return null;
    dashboards.push({ id, name, position: { x, y, w, h }, layout, activeClaims, passiveClaims });
  }
  return dashboards;
}

export function encodeTrackingSetup(dashboards: SharedDashboard[]): string | null {
  const value = [
    1,
    dashboards.map(({ id, name, position, layout, activeClaims, passiveClaims }) => [
      id,
      name,
      [position.x, position.y, position.w, position.h],
      layout.blocks.map(({ id, x, y, w, h }) => [id, x, y, w, h]),
      activeClaims,
      passiveClaims,
    ]),
  ];
  if (!parseSetup(value)) return null;
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  if (bytes.length > maxLength) return null;
  const encoded = btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
  return encoded.length <= maxLength ? prefix + encoded : null;
}

export function isTrackingSetupHash(hash: string): boolean {
  return hash.startsWith(prefix);
}

export function decodeTrackingSetup(hash: string): SharedDashboard[] | null {
  if (!isTrackingSetupHash(hash) || hash.length > prefix.length + maxLength) return null;
  const encoded = hash.slice(prefix.length);
  if (!/^[A-Za-z0-9_-]+$/.test(encoded)) return null;
  try {
    const decoded = atob(encoded.replace(/-/g, "+").replace(/_/g, "/"));
    const bytes = Uint8Array.from(decoded, (character) => character.charCodeAt(0));
    return parseSetup(JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes)));
  } catch {
    return null;
  }
}
