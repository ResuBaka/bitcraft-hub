export type CharacterStatsSnapshot = {
  player_id: string;
  revision: number | null;
  values: (number | null)[] | null;
};

export type ToolPowers = {
  slots_by_skill: Record<number, number | undefined>;
  power_by_item: Record<number, number | undefined>;
};
