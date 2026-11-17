type Station = { name: string; id: string; functionTypes: number[] };

type ProjectOwner = { is_member: boolean; owner_name: string | null; owner_id: string };

export function compareProjectOwners(a: ProjectOwner, b: ProjectOwner): number {
  return Number(b.is_member) - Number(a.is_member) ||
    (a.owner_name ?? a.owner_id).localeCompare(b.owner_name ?? b.owner_id, undefined, { sensitivity: "base", numeric: true }) ||
    a.owner_id.localeCompare(b.owner_id, undefined, { numeric: true });
}

export function compareStations(a: Station, b: Station): number {
  // Missing descriptions sort last, rather than mixing them with known types.
  if (!a.functionTypes.length && b.functionTypes.length) return 1;
  if (a.functionTypes.length && !b.functionTypes.length) return -1;
  for (let i = 0; i < Math.min(a.functionTypes.length, b.functionTypes.length); i++) {
    const difference = a.functionTypes[i]! - b.functionTypes[i]!;
    if (difference) return difference;
  }
  return a.functionTypes.length - b.functionTypes.length ||
    a.name.localeCompare(b.name, undefined, { sensitivity: "base", numeric: true }) ||
    a.id.localeCompare(b.id, undefined, { numeric: true });
}
