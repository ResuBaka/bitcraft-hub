export type PlayerCraft = {
  id: string;
  kind: "Active" | "Passive";
  status: string;
  public: boolean | null;
  building_id: string;
  building_name: string;
  building_context: { claim_id: string | null; claim_name: string | null } | null;
  recipe_id: number;
  recipe_name: string | null;
  outputs: { name: string; icon_asset_name: string; tier: number | null }[];
  quantity: number;
  progress: number | null;
  progress_total: number | bigint | null;
  slot: number | bigint | null;
  timestamp: string | null;
  duration_seconds: number | null;
};
