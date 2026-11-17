type Project = {
  id: string;
  kind: "Active" | "Passive";
  recipe_id: number;
  owner_id: string;
  status: string;
};

/** Group passive projects only when their owner, recipe and status agree. */
export function groupCraftRows<T extends Project>(crafts: T[]): T[][] {
  const rows: T[][] = [];
  const passive = new Map<string, T[]>();
  for (const craft of crafts) {
    if (craft.kind === "Active") {
      rows.push([craft]);
      continue;
    }
    const key = JSON.stringify([craft.recipe_id, craft.owner_id, craft.status]);
    let row = passive.get(key);
    if (!row) {
      row = [];
      passive.set(key, row);
      rows.push(row);
    }
    row.push(craft);
  }
  return rows;
}
