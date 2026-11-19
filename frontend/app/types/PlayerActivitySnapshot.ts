export type PlayerActivitySnapshot = {
  player_id: string;
  available: boolean;
  signed_in: boolean;
  walking: boolean | null;
  server_time_ms: number | bigint;
  actions:
    | {
        action_type: string;
        successful: boolean;
        consumed: boolean;
        start_ms: number | bigint;
        duration_ms: number | bigint;
        recipe_id: number | null;
        target_id?: string | null;
      }[]
    | null;
};
