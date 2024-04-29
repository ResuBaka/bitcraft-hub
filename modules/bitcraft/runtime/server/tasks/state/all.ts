import { readFile, writeFile } from "node:fs/promises";
import { createWriteStream } from "node:fs";
import { finished } from "node:stream/promises";
import { Readable } from "node:stream";
import {SQLRequestStream} from "./../../../SQLRequest";
let rootFolder = `${process.cwd()}/storage/State`;
let allDescTables = [  "ActiveBuffState",
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
"EnemyAiAgentState",
"EnemyState",
"EntityCombatSessionsState",
"EquipmentState",
"ExperienceState",
"ExplorationChunksState",
"FootprintTileState",
"GrowthState",
"HealthState",
"HungerState",
"InteriorCollapseTriggerState",
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
"LightStampState",
"LiveTargetableState",
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
"PlayersInQuadState",
"PortalState",
"ProgressiveActionState",
"ProjectSiteState",
"QuestState",
"RentState",
"ResourceState",
"SignedInPlayerState",
"SignedInUserState",
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
"VehicleState"
]
export default defineTask({
    meta: {
        name: "fetch:all:state",
        description: "Run database migrations",
    },
    async run({ payload, context }) {
        for (var descTable of allDescTables) {
            const sql = `SELECT * FROM ${descTable}`
            const result = await SQLRequestStream(sql)
            const stream = createWriteStream(`${rootFolder}/${descTable}.json`)
            await finished(Readable.fromWeb(result).pipe(stream))
        }

        return { result: "Success" };
    },
});
//640
