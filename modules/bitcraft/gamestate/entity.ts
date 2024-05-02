export interface Entity {
  entity_id: number;
}

export function getSome(item: any[]) {
  if (Object.keys(item)[0] === "0") {
    return Object.values(item)[0];
  }
}
