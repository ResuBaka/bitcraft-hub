<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { toast } from "vuetify-sonner";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { TravelerTaskDesc } from "~/types/TravelerTaskDesc";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { PlayerLeaderboardResponse } from "~/types/PlayerLeaderboardResponse";
import type { FindPlayerByIdResponse } from "~/types/FindPlayerByIdResponse";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { RankType } from "~/types/RankType";

const theme = useTheme();
const page = ref(1);
const route = useRoute();
const numberFormat = new Intl.NumberFormat(undefined);
const nDate = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

const tmpPage = (route.query.page as string) ?? null;

const topics = reactive<string[]>([`experience.${route.params.id}`]);
const topics_total_experience = reactive<string[]>([
  `total_experience.${route.params.id}`,
]);

let levelMap = {
  1: 0,
  2: 640,
  3: 1_340,
  4: 2_130,
  5: 2_990,
  6: 3_950,
  7: 5_000,
  8: 6_170,
  9: 7_470,
  10: 8_900,
  11: 10_480,
  12: 12_230,
  13: 14_160,
  14: 16_300,
  15: 18_660,
  16: 21_280,
  17: 24_170,
  18: 27_360,
  19: 30_900,
  20: 34_800,
  21: 39_120,
  22: 43_900,
  23: 49_180,
  24: 55_020,
  25: 61_480,
  26: 68_620,
  27: 76_520,
  28: 85_250,
  29: 94_900,
  30: 105_580,
  31: 117_380,
  32: 130_430,
  33: 144_870,
  34: 160_820,
  35: 178_470,
  36: 197_980,
  37: 219_550,
  38: 243_400,
  39: 269_780,
  40: 298_940,
  41: 331_190,
  42: 366_850,
  43: 406_280,
  44: 449_870,
  45: 498_080,
  46: 551_380,
  47: 610_320,
  48: 675_490,
  49: 747_550,
  50: 827_230,
  51: 915_340,
  52: 1_012_760,
  53: 1_120_480,
  54: 1_239_590,
  55: 1_371_290,
  56: 1_516_920,
  57: 1_677_940,
  58: 1_855_990,
  59: 2_052_870,
  60: 2_270_560,
  61: 2_511_270,
  62: 2_777_430,
  63: 3_071_730,
  64: 3_397_150,
  65: 3_756_970,
  66: 4_154_840,
  67: 4_594_770,
  68: 5_081_220,
  69: 5_619_100,
  70: 6_213_850,
  71: 6_871_490,
  72: 7_596_660,
  73: 8_394_710,
  74: 9_268_520,
  75: 10_223_770,
  76: 11_361_840,
  77: 12_563_780,
  78: 13_892_800,
  79: 15_362_330,
  80: 16_987_240,
  81: 18_783_950,
  82: 20_770_630,
  83: 22_967_360,
  84: 25_396_360,
  85: 28_082_170,
  86: 31_051_960,
  87: 34_335_740,
  88: 37_966_720,
  89: 41_981_610,
  90: 46_421_000,
  91: 51_329_760,
  92: 56_757_530,
  93: 62_759_190,
  94: 69_394_400,
  95: 76_729_260,
  96: 84_836_300,
  97: 93_794_960,
  98: 103_692_650,
  99: 114_626_640,
  100: 126_704_730,
  101: 140_247_530,
  102: 155_076_640,
  103: 171_473_630,
  104: 189_604_290,
  105: 209_651_920,
  106: 231_819_190,
  107: 256_330_230,
  108: 283_432_830,
  109: 313_401_010,
  110: 346_537_750,
};

let expUntilNextLevel = (skill) => {
  let currentLevel = skill.level;
  let currentExperience = skill.experience;
  let nextLevel = currentLevel + 1;
  let nextLevelExperience = levelMap[nextLevel];
  let experienceUntilNextLevel = nextLevelExperience - currentExperience;
  return experienceUntilNextLevel;
};

let mobileEntityStateTopics = computed(() => {
  return [`mobile_entity_state.${route.params.id}`];
});

registerWebsocketMessageHandler(
  "MobileEntityState",
  mobileEntityStateTopics,
  (message) => {
    if (playerData.value) {
      playerData.value.player_location = message;
    }
  },
);

let playerActionStateTopics = computed(() => {
  return [`player_action_state_change_name.${route.params.id}`];
});

registerWebsocketMessageHandler(
  "PlayerActionStateChangeName",
  playerActionStateTopics,
  (message) => {
    if (playerData.value) {
      playerData.value.player_action_state = message[0];
    }
  },
);

registerWebsocketMessageHandler("Experience", topics, (message) => {
  if (experienceData.value && experienceData.value[message.skill_name]) {
    let currentLevel = experienceData.value[message.skill_name].level;

    experienceData.value[message.skill_name].experience = message.experience;
    experienceData.value[message.skill_name].level = message.level;

    if (currentLevel !== message.level && currentLevel <= message.level) {
      toast(`Level ${message.level} reached for Skill ${message.skill_name}`, {
        progressBar: true,
        duration: 5000,
      });

      experienceData.value["Level"].level += 1;
    }
  }
});

registerWebsocketMessageHandler(
  "TotalExperience",
  topics_total_experience,
  (message) => {
    if (experienceData.value && experienceData.value["Experience"]) {
      experienceData.value["Experience"].experience = message.experience;
      experienceData.value["Experience"].rank = message.rank;
    }
  },
);

const topicsPlayer = reactive<string[]>([`player_state.${route.params.id}`]);

registerWebsocketMessageHandler("PlayerState", topicsPlayer, (message) => {
  if (playerData.value) {
    if (playerData.value.signed_in !== message.signed_in) {
      if (message.signed_in) {
        toast(`${playerData.value?.username} signed in`, {
          progressBar: true,
          duration: 5000,
        });
      } else {
        toast(`${playerData.value?.username} signed out`, {
          progressBar: true,
          duration: 5000,
        });
      }
    }

    playerData.value = {
      ...playerData.value,
      signed_in: message.signed_in,
      time_signed_in: message.time_signed_in,
      time_played: message.time_played,
    };
  }
});

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: playerData, pending: playerPending } =
  useFetchMsPack<FindPlayerByIdResponse>(() => {
    return `/api/bitcraft/players/${route.params.id}`;
  });

const { data: inventoryData, pending: inventoryPending } =
  useFetchMsPack<InventorysResponse>(
    () => {
      return `/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
    },
    { deep: true },
  );

let inventoryUpdateTopics = computed(() => {
  if (!inventoryData.value) {
    return [];
  }

  return inventoryData.value?.inventorys.map(
    (inventory) => `inventory_update.${inventory.entity_id}`,
  );
});

registerWebsocketMessageHandler(
  "InventoryUpdate",
  inventoryUpdateTopics,
  (message) => {
    const index = inventoryData.value.inventorys.findIndex(
      (value) => message.resolved_inventory.entity_id == value.entity_id,
    );

    if (index != -1) {
      inventoryData.value.inventorys[index].pockets =
        message.resolved_inventory.pockets;
    }
  },
);

const { data: npcData } = useFetchMsPack(() => {
  return `/npc`;
});
const { data: trevelerTasksData } = useFetchMsPack<{
  [key: number]: TravelerTaskDesc;
}>(() => {
  return `/traveler_tasks`;
});

const { data: itemsAndCargoAllData } = useFetchMsPack<ItemsAndCargollResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo/all`;
  },
);

const { data: experienceData } = useFetchMsPack<PlayerLeaderboardResponse>(
  () => {
    return `/api/bitcraft/experience/${route.params.id}`;
  },
  { deep: true },
);

const expeirence = computed(() => {
  if (!experienceData.value) {
    return undefined;
  }

  let newExperience: Record<
    string,
    RankType & {
      classes: Record<string, string>;
    }
  > = {};

  for (const [skill, xp_info] of Object.entries(experienceData.value)) {
    let shouldAddClass = true;

    if (skill === "Experience" || skill === "Level") {
      shouldAddClass = false;
    }

    newExperience[skill] = {
      experience: xp_info.experience,
      level: xp_info.level,
      rank: xp_info.rank,
      classes: {
        list: shouldAddClass
          ? `background-tier-${levelToTier(xp_info.level)}`
          : "",
        container: shouldAddClass ? "container" : "",
        content: shouldAddClass ? "content" : "",
      },
    };
  }

  return newExperience;
});
const playerInventory = computed(() => {
  return (
    inventoryData.value?.inventorys.filter(
      (inventory) =>
        inventory.nickname !== "Tool belt" &&
        inventory.nickname !== "Wallet" &&
        inventory.nickname !== "Inventory" &&
        !!inventory.pockets.filter((pocket) => !!pocket.contents?.quantity)
          .length,
    ) ?? []
  );
});

const tools = computed(() => {
  return (
    inventoryData.value?.inventorys.find(
      (inventory) => inventory.nickname === "Tool belt",
    ) ?? undefined
  );
});

const wallet = computed(() => {
  return (
    inventoryData.value?.inventorys.find(
      (inventory) => inventory.nickname === "Wallet",
    ) ?? undefined
  );
});

const mainInventory = computed(() => {
  return (
    inventoryData.value?.inventorys.find(
      (inventory) => inventory.nickname === "Inventory",
    ) ?? undefined
  );
});

const deployables = computed(() => {
  return playerData.value?.deployables ?? undefined;
});

const levelToTier = (level: number) => {
  if (1 <= level && level <= 19) {
    return 1;
  }
  if (20 <= level && level <= 29) {
    return 2;
  }
  if (30 <= level && level <= 39) {
    return 3;
  }
  if (40 <= level && level <= 49) {
    return 4;
  }
  if (50 <= level && level <= 59) {
    return 5;
  }
  if (60 <= level && level <= 69) {
    return 6;
  }
  if (70 <= level && level <= 79) {
    return 7;
  }
  if (80 <= level && level <= 89) {
    return 8;
  }
  if (90 <= level && level <= 99) {
    return 9;
  }
  if (100 === level) {
    return 10;
  }
};

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

const skillToToolIndex = {
  Carpentry: 1,
  Construction: 13,
  Cooking: 10,
  Experience: undefined,
  Farming: 8,
  Fishing: 9,
  Foraging: 11,
  Forestry: 0,
  Hunting: 6,
  Leatherworking: 5,
  Level: undefined,
  Masonry: 2,
  Mining: 3,
  Scholar: 12,
  Slayer: 14,
  Smithing: 4,
  Tailoring: 7,
};

const tierColor = useTierColor();

const secondsToDaysMinutesSecondsFormat = (seconds: number) => {
  const days = Math.floor(seconds / (60 * 60 * 24));
  const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  const minutes = Math.floor((seconds % (60 * 60)) / 60);
  const secondsLeft = seconds % 60;

  let result = "";

  if (days > 0) {
    result += `${days}d `;
  }

  if (hours > 0) {
    result += `${hours}h `;
  }

  if (minutes > 0) {
    result += `${minutes}m `;
  }

  if (secondsLeft > 0) {
    result += `${secondsLeft}s`;
  }

  return result;
};

const iconUrl = (item: any) => {
  if (!item?.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(item.icon_asset_name);
};

useSeoMeta({
  title: () => `Player ${playerData.value?.username ?? route.params.id}`,
  description: () => `Player ${playerData.value?.username ?? route.params.id}`,
});
</script>

<template>
  <v-container fluid>
    <v-layout class="justify-center" v-if="playerPending">
      <v-progress-circular indeterminate>
      </v-progress-circular>
    </v-layout>
    <template v-else-if="playerData">
      <v-banner :class="`text-decoration-none font-weight-black ${playerData.signed_in ? 'text-green' : 'text-high-emphasis'}`">Player: {{ playerData?.username }}</v-banner>
      <v-card>
        <v-card-text :class="computedClass">
          <v-table :class="computedClass" density="compact">
            <tbody>
            <tr style='text-align: right'>
              <th>Played:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(playerData.time_played) }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Signed in:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(playerData.time_signed_in) }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Signed in:</th>
              <td>{{ nDate.format(new Date(playerData.sign_in_timestamp * 1000)) }}</td>
            </tr>
            <tr v-if="playerData.player_location" style='text-align: right'>
              <th>Location:</th>
              <td>N: {{ Math.floor(playerData.player_location?.location_z / 3 / 1000) }} E: {{ Math.floor(playerData.player_location?.location_x / 3 / 1000) }} R: <bitcraft-region v-if="playerData.player_location?.region" :region="playerData.player_location?.region" /></td>
            </tr>
            <tr style='text-align: right'>
              <th>Current Action:</th>
              <td>{{ playerData.player_action_state ?? "" }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Hex Coins:</th>
              <td>{{ wallet?.pockets[0]?.contents?.quantity ?? 0 }}</td>
            </tr>
            <tr style='text-align: right' v-if="playerData?.claims?.length">
              <th>Claims:</th>
              <td>
                <nuxt-link class="text-decoration-none font-weight-black text-high-emphasis"
                           :to="{ name: 'claims-id', params: { id: claim.entity_id.toString() } }"
                           v-for="(claim, index) in playerData?.claims"
                >{{ claim.name.toString() }}{{ index === (playerData?.claims?.length - 1) ? '' : ', ' }}
                </nuxt-link>
              </td>
            </tr>
            </tbody>
          </v-table>
          <v-row>
            <v-col cols="12">
              <v-card variant="text" v-if="deployables !== undefined && deployables.length">
                <v-card-title>Deployable</v-card-title>
                <v-card-text>
                  <v-row>
                    <v-col cols="12" md="4" lg="2" v-for="deployable in deployables" :key="deployable.id">
                      <v-list>
                        <v-list-item>
                            <v-list-item-title>{{ deployable.collectible_desc.name }}</v-list-item-title>
                            <v-list-item-subtitle>Amount: {{ deployable.count }}</v-list-item-subtitle>
                            <v-list-item-subtitle>{{ deployable.activated ? "Activated" : "Not Activated" }}</v-list-item-subtitle>
                        </v-list-item>
                      </v-list>
                    </v-col>
                  </v-row>
                </v-card-text>
              </v-card>
            </v-col>
          </v-row>
          <v-row v-if="expeirence !== undefined">
            <v-col cols="12">
              <v-card variant="text">
                <v-card-title>Skills</v-card-title>
                <v-card-text>
                  <v-row>
                    <v-col cols="12" md="4" sm="4" xxl="2" xl="2" lg="2" v-for="[skill,xp_info] of Object.entries(expeirence)" :key="skill">
                      <v-list :class="xp_info.classes.container">
                        <div :class="xp_info.classes.list"></div>
                        <v-row dense no-gutters :class="xp_info.classes.content">
                          <v-col>
                          <v-list-item >
                            <v-list-item-title>{{ skill }}</v-list-item-title>
                            <v-list-item-subtitle v-if="!['Level'].includes(skill)">Experience: <bitcraft-animated-number :value="xp_info.experience" :speed="8" :formater="numberFormat.format"></bitcraft-animated-number></v-list-item-subtitle>
                            <v-list-item-subtitle v-else>&nbsp;</v-list-item-subtitle>
                            <v-list-item-subtitle v-if="!['Level', 'Experience'].includes(skill)">To next: <bitcraft-animated-number :value="expUntilNextLevel(xp_info)" :speed="8" :formater="numberFormat.format"></bitcraft-animated-number></v-list-item-subtitle>
                            <v-list-item-subtitle v-else>&nbsp;</v-list-item-subtitle>
                            <v-list-item-subtitle v-if="!['Experience'].includes(skill)">Level: <bitcraft-animated-number v-if="xp_info.level" :value="xp_info.level" :speed="50"></bitcraft-animated-number></v-list-item-subtitle>
                            <v-list-item-subtitle v-else>&nbsp;</v-list-item-subtitle>
                            <v-list-item-subtitle>Rank: #<bitcraft-animated-number :value="xp_info.rank" :speed="50"></bitcraft-animated-number></v-list-item-subtitle>
                          </v-list-item>
                          </v-col>
                          <v-col cols="4" md="4" xs="4" v-if="skillToToolIndex[skill] >= 0 && tools?.pockets[skillToToolIndex[skill]].contents" :class="`text-${tierColor[tools?.pockets[skillToToolIndex[skill]].contents.item.tier]}`">
                            {{ tools.pockets[skillToToolIndex[skill]].contents.item.name ?? "No tool" }} {{ tools.pockets[skillToToolIndex[skill]].contents.item.rarity ?? "" }}
                            <v-img :src="iconUrl(tools.pockets[skillToToolIndex[skill]].contents.item).url" height="50" width="50"></v-img>
                          </v-col>
                        </v-row>
                      </v-list>
                    </v-col>
                  </v-row>
                </v-card-text>
              </v-card>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>
      <v-expansion-panels>
        <v-expansion-panel>
          <v-expansion-panel-title>
            <v-row>
              <v-col class="d-flex justify-center">
                <h2 class="pl-md-3 pl-xl-0">Treveler tasks</h2>
              </v-col>
            </v-row>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <template v-for="(traveler, index) of playerData.traveler_tasks" >
              <v-row>
                <v-col class="d-flex justify-center font-weight-bold">
                {{ npcData[index]?.name }}
                </v-col>
              </v-row>
              <v-row>
              <template v-for="task of traveler">
                  <v-col class="d-flex justify-center">
                    <template v-for="item of trevelerTasksData[task.task_id]?.required_items">
                      <div class="align-content-center">{{  }}</div>
                      <div class="align-content-center" :class="`text-${tierColor[item.item_type == 'Item' ? itemsAndCargoAllData.item_desc[item.item_id].tier : itemsAndCargoAllData.cargo_desc[item.item_id].tier]}`">
                        <template v-if="item.item_type == 'Item'">
                          {{ itemsAndCargoAllData.item_desc[item.item_id].name }}
                        </template>
                        <template v-else-if="item.item_type == 'Cargo'">
                          {{ itemsAndCargoAllData.cargo_desc[item.item_id].name }}
                        </template>
                      </div>
                      <v-badge :content="Intl.NumberFormat().format(item.quantity)" :color="task.completed ? 'green' : 'red'" location="right" class="align-start">
                        <template v-if="item.item_type == 'Item'">
                          <v-img :src="iconAssetUrlNameRandom(itemsAndCargoAllData.item_desc[item.item_id].icon_asset_name).url" height="75" :width="item.type == 'Item' ? 75 : 128"></v-img>
                        </template>
                        <template v-else-if="item.item_type == 'Cargo'">
                          <v-img :src="iconAssetUrlNameRandom(itemsAndCargoAllData.cargo_desc[item.item_id].icon_asset_name).url" height="75" :width="item.type == 'Item' ? 75 : 128"></v-img>
                        </template>
                      </v-badge>
                    </template>
                  </v-col>
              </template>
              </v-row>
            </template>
          </v-expansion-panel-text>
        </v-expansion-panel>
      </v-expansion-panels>
      <v-card variant="text" v-if="playerInventory.length || tools || mainInventory">
        <v-card-title>Inventory's</v-card-title>
        <v-card-text>
          <v-row>
<!--            <v-col cols="12" md="6" v-if="playerTools">-->
<!--              <bitcraft-playerData-tool-belt :inventory="playerTools"></bitcraft-playerData-tool-belt>-->
<!--            </v-col>-->
            <v-col cols="12" md="6" v-if="mainInventory">
              <bitcraft-inventory :inventory="mainInventory"></bitcraft-inventory>
            </v-col>

            <template v-if="!inventoryPending" v-for="(inventory, index) in playerInventory">
              <v-col cols="12" md="6">
                <bitcraft-inventory :inventory="inventory"></bitcraft-inventory>
              </v-col>
            </template>
            <v-layout class="justify-center" v-else>
              <v-progress-circular indeterminate>
              </v-progress-circular>
            </v-layout>
          </v-row>
        </v-card-text>
      </v-card>
    </template>
    <template v-else>
      <v-alert type="error">Player not found</v-alert>
    </template>
  </v-container>
</template>

<style scoped>
</style>