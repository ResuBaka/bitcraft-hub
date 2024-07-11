import { writeFile } from "node:fs/promises";
import SQLRequest from "./../../../SQLRequest";
import {
  parseInventorys,
  readInventoryRows,
  saveParsedInventorys,
} from "../../../../gamestate/inventory";
let rootFolder = `${process.cwd()}/storage/Desc`;
let allDescTables = [
  "AchievementDesc",
  "AlertDesc",
  //"BiomeDesc",
  "BuffDesc",
  "BuildingClaimDesc",
  "BuildingDesc",
  "BuildingPortalDesc",
  "BuildingRepairsDesc",
  // "BuildingSpawnDesc",
  "BuildingTypeDesc",
  "CargoDesc",
  "CharacterStatDesc",
  //"ChestRarityDesc",
  "ClaimTechDesc",
  "ClimbRequirementDesc",
  "ClothingDesc",
  "CollectibleDesc",
  "CombatActionDesc",
  "ConstructionRecipeDesc",
  "CraftingRecipeDesc",
  "DeconstructionRecipeDesc",
  "EmoteDesc",
  //"EnemyAiParamsDesc",
  "EnemyDesc",
  "EnvironmentDebuffDesc",
  "EquipmentDesc",
  "ExtractionRecipeDesc",
  "FoodDesc",
  "InteriorInstanceDesc",
  "InteriorNetworkDesc",
  "InteriorPortalConnectionsDesc",
  "InteriorShapeDesc",
  "InteriorSpawnDesc",
  "ItemConversionRecipeDesc",
  "ItemDesc",
  "ItemListDesc",
  "KnowledgeScrollDesc",
  //"KnowledgeScrollTypeDesc",
  "LootChestDesc",
  //"LootRarityDesc",
  "LootTableDesc",
  "NpcDesc",
  //"OnboardingRewardDesc",
  "ParametersDesc",
  "PavingTileDesc",
  //"ResourceClumpDesc",
  "ResourceDesc",
  //"ResourceGrowthRecipeDesc",
  "ResourcePlacementRecipeDesc",
  //"SecondaryKnowledgeDesc",
  "SkillDesc",
  //"TargetingMatrixDesc",
  "TeleportItemDesc",
  "TerraformRecipeDesc",
  "ToolDesc",
  "ToolTypeDesc",
  "TravelerTradeOrderDesc",
  "VehicleDesc",
  "WeaponDesc",
  "WeaponTypeDesc",
  "EmpireColorDesc",
  "EmpireNotificationDesc",
  "EmpireRankDesc",
  "EmpireSuppliesDesc",
  "EmpireTerritoryDesc",
];
export default defineTask({
  meta: {
    name: "fetch:all:desc",
    description: "Run database migrations",
  },
  async run({ payload, context }) {
    for (var descTable of allDescTables) {
      console.log(descTable);
      try {
        const sql = `SELECT * FROM ${descTable}`;
        const result = await SQLRequest<any>(sql);
        await writeFile(
          `${rootFolder}/${descTable}.json`,
          JSON.stringify(result),
        );
      } catch (error) {
        console.error(error);
      }
    }

    const inventoryRows = readInventoryRows();
    const parsedInventoryRows = parseInventorys(inventoryRows);
    saveParsedInventorys(parsedInventoryRows);

    return { result: "Success" };
  },
});
