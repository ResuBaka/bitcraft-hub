<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { toast } from "vuetify-sonner";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { TravelerTaskDesc } from "~/types/TravelerTaskDesc";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { PlayerLeaderboardResponse } from "~/types/PlayerLeaderboardResponse";
import type { FindPlayerByIdResponse } from "~/types/FindPlayerByIdResponse";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { HouseResponse } from "~/types/HouseResponse";
import type { RankType } from "~/types/RankType";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";

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
  2: 520,
  3: 1100,
  4: 1740,
  5: 2460,
  6: 3270,
  7: 4170,
  8: 5170,
  9: 6290,
  10: 7540,
  11: 8930,
  12: 10490,
  13: 12220,
  14: 14160,
  15: 16320,
  16: 18730,
  17: 21420,
  18: 24410,
  19: 27760,
  20: 31490,
  21: 35660,
  22: 40310,
  23: 45490,
  24: 51280,
  25: 57740,
  26: 64940,
  27: 72980,
  28: 81940,
  29: 91950,
  30: 103110,
  31: 115560,
  32: 129460,
  33: 144960,
  34: 162260,
  35: 181560,
  36: 203100,
  37: 227130,
  38: 253930,
  39: 283840,
  40: 317220,
  41: 354450,
  42: 396000,
  43: 442350,
  44: 494070,
  45: 551770,
  46: 616150,
  47: 687980,
  48: 768130,
  49: 857560,
  50: 957330,
  51: 1068650,
  52: 1192860,
  53: 1331440,
  54: 1486060,
  55: 1658570,
  56: 1851060,
  57: 2065820,
  58: 2305430,
  59: 2572780,
  60: 2871080,
  61: 3203890,
  62: 3575230,
  63: 3989550,
  64: 4451810,
  65: 4967590,
  66: 5543050,
  67: 6185120,
  68: 6901500,
  69: 7700800,
  70: 8592610,
  71: 9587630,
  72: 10697810,
  73: 11936490,
  74: 13318540,
  75: 14860540,
  76: 16581010,
  77: 18500600,
  78: 20642370,
  79: 23032020,
  80: 25698250,
  81: 28673070,
  82: 31992200,
  83: 35695470,
  84: 39827360,
  85: 44437480,
  86: 49581160,
  87: 55320170,
  88: 61723410,
  89: 68867770,
  90: 76839000,
  91: 85732810,
  92: 95656000,
  93: 106727680,
  94: 119080790,
  95: 132863630,
  96: 148241700,
  97: 165399620,
  98: 184543380,
  99: 205902840,
  100: 229734400,
  101: 256324240,
  102: 285991580,
  103: 319092580,
  104: 356024680,
  105: 397231240,
  106: 443207040,
  107: 494504080,
  108: 551738200,
  109: 615596560,
  110: 686845760,
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

const { data: houses, pending: housesPending } = await useLazyFetchMsPack<
  HouseResponse[]
>(() => `/api/bitcraft/houses/by_owner/${route.params.id}`);

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

let inventoryRemoveTopics = computed(() => {
  if (!inventoryData.value) {
    return [];
  }

  return inventoryData.value?.inventorys.map(
    (inventory) => `inventory_remove.${inventory.entity_id}`,
  );
});

registerWebsocketMessageHandler(
  "InventoryRemove",
  inventoryRemoveTopics,
  (message) => {
    const index = inventoryData.value.inventorys.findIndex(
      (value) => message.resolved_inventory.entity_id == value.entity_id,
    );

    if (index != -1) {
      inventoryData.value.inventorys.splice(index, 1);
    }
  },
);

registerWebsocketMessageHandler(
  "InventoryInsert",
  [`inventory_insert_player_owner.${route.params.id}`],
  (message) => {
    const index = inventoryData.value.inventorys.findIndex(
      (value) => message.resolved_inventory.entity_id == value.entity_id,
    );

    if (index != -1) {
      inventoryData.value.inventorys[index].pockets =
        message.resolved_inventory.pockets;
    } else {
      inventoryData.value.inventorys.push(message.resolved_inventory);
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
  Slayer: 15,
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

const getRankName = (rank: number) => {
  switch (rank) {
    case 7:
      return "Owner";
    case 6:
      return "Admin";
    case 5:
      return "Resident";
    case 1:
      return "Guest";
    default:
      return `Rank ${rank}`;
  }
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
      <v-sheet
          class="d-flex align-center justify-center flex-wrap text-center mx-auto px-4"
          elevation="4"
          height="110"
          width="100%"
      >
        <div>
          <h2 class="text-h5 font-weight-black" :class="`${playerData.signed_in ? 'text-green' : 'text-high-emphasis'}`">{{ playerData?.username }}</h2>
          <div v-if="playerData.player_location">
            <p class="text-body-2">N: {{ Math.floor(playerData.player_location?.location_z / 3 / 1000) }} E: {{ Math.floor(playerData.player_location?.location_x / 3 / 1000) }} R: <bitcraft-region v-if="playerData.player_location?.region" :region="playerData.player_location?.region" /></p>
          </div>
          <div class="d-flex flex-wrap justify-center ga-1">
            <v-chip rounded="1">
              Played: {{ secondsToDaysMinutesSecondsFormat(playerData.time_played) }}
            </v-chip>
            <v-chip rounded="1">
              Signed in: {{ secondsToDaysMinutesSecondsFormat(playerData.time_signed_in) }}
            </v-chip>
            <v-chip rounded="1" v-if="new Date(playerData.sign_in_timestamp * 1000).getFullYear() !== 1970">
              Login at: {{ nDate.format(new Date(playerData.sign_in_timestamp * 1000)) }}
            </v-chip>
            <v-chip rounded="1" color="yellow">
              Hex Coins: {{ numberFormat.format(wallet?.pockets[0]?.contents?.quantity ?? 0) }}
            </v-chip>
          </div>
        </div>
      </v-sheet>
      <v-card>
        <v-card-text :class="computedClass">
          <v-table :class="computedClass" density="compact">
            <tbody>
            <tr style='text-align: right' v-if="playerData.player_action_state">
              <th>Current Action:</th>
              <td>{{ playerData.player_action_state ?? "" }}</td>
            </tr>
            <tr v-if="playerData?.claims?.length">
              <th>Claims:</th>
              <td>
                <v-chip-group column class="justify-end-chips">
                  <nuxt-link class="text-decoration-none font-weight-black text-high-emphasis"
                             :to="{ name: 'claims-id', params: { id: claim.entity_id.toString() } }"
                             v-for="(claim, index) in playerData?.claims"
                  >
                    <v-chip rounded="1">
                      {{ claim.name.toString() }}
                    </v-chip>
                  </nuxt-link>
                </v-chip-group>
              </td>
            </tr>
            </tbody>
          </v-table>
          <v-row>
            <v-col cols="12">
              <v-card variant="text" v-if="deployables !== undefined && deployables.length">
                <v-card-title>Deployable</v-card-title>
                <v-card-text>
                  <div class="d-flex flex-wrap ga-1">
                  <div v-for="deployable in deployables" :key="deployable.id">
                      <v-list>
                        <v-list-item>
                            <v-list-item-title>{{ deployable.collectible_desc.name }}</v-list-item-title>
                            <v-list-item-subtitle>Amount: {{ deployable.count }}</v-list-item-subtitle>
                            <v-list-item-subtitle>{{ deployable.activated ? "Activated" : "Not Activated" }}</v-list-item-subtitle>
                        </v-list-item>
                      </v-list>
                    </div>
                  </div>
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
                    <v-col cols="12" md="6" xxl="2" xl="3" lg="4" v-for="[skill,xp_info] of Object.entries(expeirence)" :key="skill">
                      <v-list :class="xp_info.classes.container">
                        <div :class="xp_info.classes.list"></div>
                        <v-row dense no-gutters :class="xp_info.classes.content">
                          <v-col cols="8">
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
                          <v-spacer />
                          <v-col v-if="skillToToolIndex[skill] >= 0 && tools?.pockets[skillToToolIndex[skill]].contents" class="pr-2">
                            <v-sheet
                                rounded
                                class="inventory-slot-box d-flex align-center justify-center position-relative border-lg"
                                :class="`bg-color-tier-${tools?.pockets[skillToToolIndex[skill]].contents.item.tier} border-color-rarity-${tools?.pockets[skillToToolIndex[skill]].contents.item.rarity.toLowerCase()}`"
                            >
                              <v-tooltip activator="parent" location="top" transition="fade-transition">
                                <div class="text-center">
                                  <div :class="`font-weight-bold text-${getTierColor(tools?.pockets[skillToToolIndex[skill]].contents.item.tier)} text-uppercase`">
                                    {{ tools?.pockets[skillToToolIndex[skill]].contents.item.name }}
                                  </div>
                                  <div class="text-caption">Rarity: {{ tools?.pockets[skillToToolIndex[skill]].contents.item.rarity }}</div>
                                </div>
                              </v-tooltip>

                              <div class="tier-label" :class="`text-${getTierColor(tools?.pockets[skillToToolIndex[skill]].contents.item.tier)}`">
                                T{{ tools?.pockets[skillToToolIndex[skill]].contents.item.tier }}
                              </div>

                              <div class="item-icon text-h6 font-weight-black">
                                <inventory-img :item="tools.pockets[skillToToolIndex[skill]].contents.item" aspect-ratio="1"/>
                              </div>
                            </v-sheet>
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
        <v-expansion-panel v-if="houses && houses.length">
          <v-expansion-panel-title>
            <v-row>
              <v-col class="d-flex justify-center">
                <h2 class="pl-md-3 pl-xl-0">Houses ({{ houses.length }})</h2>
              </v-col>
            </v-row>
          </v-expansion-panel-title>
          <v-expansion-panel-text>
            <bitcraft-house-details
              v-for="house in houses"
              :key="house.entity_id.toString()"
              :house="house"
            />
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
            <v-col cols="12" md="6"  v-if="mainInventory">
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
.justify-end-chips :deep(.v-slide-group__content) {
  justify-content: flex-end;
}

.inventory-slot-box {
  width: 100px;
  aspect-ratio: 1 / 1;
  cursor: default;
  overflow: hidden;
  transition: all 0.2s ease;
}

.item-icon {
  opacity: 0.8;
  user-select: none;
}

.tier-label {
  position: absolute;
  top: 8px; /* Adjusted to sit just below or on the 4px border */
  left: 4px;
  font-size: 0.9rem;
  font-weight: 900;
  line-height: 1;
  text-transform: uppercase;
  user-select: none;
  /* Optional: gives it a slight shadow to pop against dark icons */
  text-shadow: 0px 0px 2px rgb(var(--v-theme-surface));
}
</style>