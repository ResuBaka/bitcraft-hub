import fs, { createWriteStream } from "node:fs";
import { finished } from "node:stream/promises";
import { Readable } from "node:stream";
import SQLRequest, { SQLRequestStream } from "./../../../SQLRequest";
import { rebuildLeaderboardState } from "../../../../gamestate/experienceState";
import { writeFile } from "node:fs/promises";
let rootFolder = `${process.cwd()}/storage/State`;
let allDescTables = [
  "ActiveBuffState",
  "AlertState",
  "AttackOutcomeState",
  "BuildingState",
  "CargoState",
  "CharacterStatsState",
  "ChatMessageState",
  "ClaimDescriptionState",
  "ClaimRecruitmentState",
  "ClaimTechState",
  "ClaimTileState",
  "CombatSessionState",
  "CombatState",
  "DimensionDescriptionState",
  "DimensionNetworkDescriptionState",
  //"EnemyAiAgentState",
  "EnemyState",
  "EntityCombatSessionsState",
  "EquipmentState",
  "ExperienceState",
  "ExplorationChunksState",
  "FootprintTileState",
  //"GrowthState",
  "HealthState",
  "HungerState",
  //"InteriorCollapseTriggerState",
  "InventoryState",
  "ItemPileState",
  "KnowledgeAchievementState",
  "KnowledgeBattleActionState",
  "KnowledgeBuildingState",
  "KnowledgeCargoState",
  "KnowledgeConstructionState",
  "KnowledgeCraftState",
  "KnowledgeEnemyState",
  "KnowledgeExtractState",
  "KnowledgeItemState",
  "KnowledgeLoreState",
  "KnowledgeNpcState",
  "KnowledgePavingState",
  "KnowledgeResourcePlacementState",
  "KnowledgeResourceState",
  "KnowledgeRuinsState",
  "KnowledgeSecondaryState",
  "KnowledgeVaultState",
  "KnowledgeVehicleState",
  //"LightStampState",
  //"LiveTargetableState",
  //"LocationState",
  "LootChestState",
  "MobileEntityState",
  "MountingState",
  "NpcState",
  "OnboardingState",
  "PassiveCraftTimerState",
  "PavedTileState",
  "PlayerActionState",
  "PlayerPrefsState",
  "PlayerState",
  "PlayerVoteState",
  //"PlayersInQuadState",
  "PortalState",
  "ProgressiveActionState",
  "ProjectSiteState",
  "QuestState",
  "RentState",
  "ResourceState",
  //"SignedInPlayerState",
  //"SignedInUserState",
  "StaminaState",
  "TargetState",
  "TargetableState",
  "TerrainChunkState",
  "TimerState",
  "TradeOrderState",
  "TradeSessionState",
  "UserModerationState",
  "UserSignInState",
  "UserState",
  "VaultState",
  "VehicleState",
];
export default defineTask({
  meta: {
    name: "fetch:all:state",
    description: "Run database migrations",
  },
  async run({ payload, context }) {
    for (var descTable of allDescTables) {
      try {
        console.log(descTable);
        const sql = `SELECT * FROM ${descTable}`;

        const filePath = `${rootFolder}/${descTable}.json`;
        if (fs.existsSync(filePath)) {
          fs.unlinkSync(filePath);
          console.log("Deleted file", filePath);
        }

        const result = await SQLRequest<any>(sql);
        await writeFile(filePath, JSON.stringify(result));

        // const result = await SQLRequestStream(sql);
        // const stream = createWriteStream(filePath);
        // console.log("Writing to file", filePath);
        // await finished(Readable.fromWeb(result).pipe(stream));
      } catch (e) {
        console.error(e);
      }
    }

    console.log("Rebuilding Leaderboard");
    rebuildLeaderboardState();
    console.log("Rebuilding Leaderboard Complete");

    return { result: "Success" };
  },
});
