<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { toast } from "vuetify-sonner";
const theme = useTheme();
const page = ref(1);
const route = useRoute();
const numberFormat = new Intl.NumberFormat(undefined);

const tmpPage = (route.query.page as string) ?? null;

import { registerWebsocketMessageHandler } from "~/composables/websocket";
const useWebsocket = useWebsocketStore();

type Message = {
  t: string;
  c: undefined | Record<string, any>;
};

const topics = reactive<string[]>([`experience.${route.params.id}`]);

let levelMap = {
  1: 0,
  2: 640,
  3: 1_330,
  4: 2_090,
  5: 2_920,
  6: 3_830,
  7: 4_820,
  8: 5_890,
  9: 7_070,
  10: 8_350,
  11: 9_740,
  12: 11_260,
  13: 12_920,
  14: 14_730,
  15: 16_710,
  16: 18_860,
  17: 21_210,
  18: 23_770,
  19: 26_560,
  20: 29_600,
  21: 32_920,
  22: 36_550,
  23: 40_490,
  24: 44_800,
  25: 49_490,
  26: 54_610,
  27: 60_200,
  28: 66_290,
  29: 72_930,
  30: 80_170,
  31: 88_060,
  32: 96_670,
  33: 106_060,
  34: 116_300,
  35: 127_470,
  36: 139_650,
  37: 152_930,
  38: 167_410,
  39: 183_200,
  40: 200_420,
  41: 219_200,
  42: 239_680,
  43: 262_020,
  44: 286_370,
  45: 312_930,
  46: 341_890,
  47: 373_480,
  48: 407_920,
  49: 445_480,
  50: 486_440,
  51: 531_110,
  52: 579_820,
  53: 632_940,
  54: 690_860,
  55: 754_030,
  56: 822_920,
  57: 898_040,
  58: 979_960,
  59: 1_069_290,
  60: 1_166_710,
  61: 1_272_950,
  62: 1_388_800,
  63: 1_515_140,
  64: 1_652_910,
  65: 1_803_160,
  66: 1_967_000,
  67: 2_145_660,
  68: 2_340_500,
  69: 2_552_980,
  70: 2_784_680,
  71: 3_037_360,
  72: 3_312_900,
  73: 3_613_390,
  74: 3_941_070,
  75: 4_298_410,
  76: 4_688_090,
  77: 5_113_030,
  78: 5_576_440,
  79: 6_081_800,
  80: 6_632_890,
  81: 7_233_850,
  82: 7_889_210,
  83: 8_603_890,
  84: 9_383_250,
  85: 10_233_150,
  86: 11_159_970,
  87: 12_170_670,
  88: 13_272_850,
  89: 14_474_790,
  90: 15_785_510,
  91: 17_214_860,
  92: 18_773_580,
  93: 20_473_370,
  94: 22_327_010,
  95: 24_348_420,
  96: 26_552_780,
  97: 28_956_650,
  98: 31_578_090,
  99: 34_436_800,
  100: 37_554_230,
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
    if (playerFetch.value) {
      playerFetch.value.player_location = message.c;
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
    if (playerFetch.value) {
      playerFetch.value.player_action_state = message.c[0];
    }
  },
);

registerWebsocketMessageHandler("Experience", topics, (message) => {
  if (experienceFetch.value && experienceFetch.value[message.c.skill_name]) {
    let currentExperience =
      experienceFetch.value[message.c.skill_name].experience;
    let currentLevel = experienceFetch.value[message.c.skill_name].level;
    experienceFetch.value[message.c.skill_name] = {
      ...experienceFetch.value[message.c.skill_name],
      experience: message.c.experience,
      level: message.c.level,
    };

    if (currentLevel !== message.c.level && currentLevel <= message.c.level) {
      toast(
        `Level ${message.c.level} reached for Skill ${message.c.skill_name}`,
        { progressBar: true, duration: 5000 },
      );

      experienceFetch.value["Level"].level += 1;
    }

    if (experienceFetch.value["Experience"]) {
      let newExperience = message.c.experience;
      let increase = newExperience - currentExperience;

      experienceFetch.value["Experience"] = {
        ...experienceFetch.value["Experience"],
        experience: experienceFetch.value["Experience"].experience + increase,
      };
    }
  }
});

const topicsPlayer = reactive<string[]>([`player_state.${route.params.id}`]);

registerWebsocketMessageHandler("PlayerState", topicsPlayer, (message) => {
  if (playerFetch.value && playerFetch.value) {
    if (playerFetch.value.signed_in !== message.c.signed_in) {
      if (message.c.signed_in) {
        toast(`${player.value?.username} signed in`, {
          progressBar: true,
          duration: 5000,
        });
      } else {
        toast(`${player.value?.username} signed out`, {
          progressBar: true,
          duration: 5000,
        });
      }
    }

    playerFetch.value = {
      ...playerFetch.value,
      signed_in: message.c.signed_in,
      time_signed_in: message.c.time_signed_in,
      time_played: message.c.time_played,
    };
  }
});

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const {
  public: { api },
} = useRuntimeConfig();

const { data: playerFetch, pending: playerPnding } = useFetch(() => {
  return `${api.base}/api/bitcraft/players/${route.params.id}`;
});
const { data: inventoryFetch, pending: inventoryPending } = useFetch(() => {
  return `${api.base}/api/bitcraft/inventorys/owner_entity_id/${route.params.id}`;
});

const { data: experienceFetch } = useFetch(() => {
  return `${api.base}/api/bitcraft/experience/${route.params.id}`;
});

const expeirence = computed(() => {
  if (!experienceFetch.value) {
    return undefined;
  }

  let newExperience: Record<string, any> = {};

  for (const [skill, xp_info] of Object.entries(experienceFetch.value)) {
    let shouldAddClass = xp_info.level && xp_info.experience;

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
const inventorys = computed(() => {
  return (
    inventoryFetch.value?.inventorys.filter(
      (inventory) =>
        inventory.nickname !== "Tool belt" &&
        inventory.nickname !== "Wallet" &&
        inventory.nickname !== "Inventory",
    ) ?? []
  );
});

const playerTools = computed(() => {
  return (
    inventoryFetch.value?.inventorys.find(
      (inventory) => inventory.nickname === "Tool belt",
    ) ?? undefined
  );
});

const playerWallet = computed(() => {
  return (
    inventoryFetch.value?.inventorys.find(
      (inventory) => inventory.nickname === "Wallet",
    ) ?? undefined
  );
});

const playerInventory = computed(() => {
  return (
    inventoryFetch.value?.inventorys.find(
      (inventory) => inventory.nickname === "Inventory",
    ) ?? undefined
  );
});

const player = computed(() => {
  return playerFetch.value ?? undefined;
});

const deployables = computed(() => {
  return playerFetch.value?.deployables ?? undefined;
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
  if (70 <= level) {
    return 7;
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

const tierColor = computed(() => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  const colors = {
    1: `grey${colorEffect}`,
    2: `green${colorEffect}`,
    3: `blue${colorEffect}`,
    4: `purple${colorEffect}`,
    5: `yellow${colorEffect}`,
    6: `pink${colorEffect}`,
  };

  return colors;
});

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
  title: () => `Player ${playerFetch.value?.username ?? route.params.id}`,
  description: () => `Player ${playerFetch.value?.username ?? route.params.id}`,
});
</script>

<template>
  <v-container fluid>
    <v-layout class="justify-center" v-if="playerPnding">
      <v-progress-circular indeterminate>
      </v-progress-circular>
    </v-layout>
    <template v-else-if="player">
      <v-banner :class="`text-decoration-none font-weight-black ${player.signed_in ? 'text-green' : 'text-high-emphasis'}`">Player: {{ player?.username }}</v-banner>
      <v-card>
        <v-card-text :class="computedClass">
          <v-table :class="computedClass" density="compact">
            <tbody>
            <tr style='text-align: right'>
              <th>Played:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(player.time_played) }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Signed in:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(player.time_signed_in) }}</td>
            </tr>
            <tr v-if="player.player_location" style='text-align: right'>
              <th>Location:</th>
              <td>N: {{ Math.floor(player.player_location?.location_z / 3 / 1000) }} E: {{ Math.floor(player.player_location?.location_x / 3 / 1000) }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Current Action:</th>
              <td>{{ player.player_action_state ?? "" }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>Hex Coints:</th>
              <td>{{ playerWallet?.pockets[0].contents?.quantity ?? 0 }}</td>
            </tr>
            </tbody>
          </v-table>
          <v-row>
            <v-col cols="12">
              <v-card variant="text" v-if="deployables !== undefined && deployables.length">
                <v-card-title>Deployables</v-card-title>
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
                    <v-col cols="12" md="4" xxl="2" lg="3" v-for="[skill,xp_info] of Object.entries(expeirence)" :key="skill">
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
                          <v-col cols="4" md="4" xs="4" v-if="skillToToolIndex[skill] >= 0 && playerTools?.pockets[skillToToolIndex[skill]].contents" :class="`text-${tierColor[playerTools?.pockets[skillToToolIndex[skill]].contents.item.tier]}`">
                            {{ playerTools.pockets[skillToToolIndex[skill]].contents.item.name ?? "No tool" }}
                            <v-img :src="iconUrl(playerTools.pockets[skillToToolIndex[skill]].contents.item).url" height="50" width="50"></v-img>
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

      <v-card variant="text" v-if="inventorys.length || playerTools || playerInventory">
        <v-card-title>Inventory's</v-card-title>
        <v-card-text>
          <v-row>
<!--            <v-col cols="12" md="6" v-if="playerTools">-->
<!--              <bitcraft-player-tool-belt :inventory="playerTools"></bitcraft-player-tool-belt>-->
<!--            </v-col>-->
            <v-col cols="12" md="6" v-if="playerInventory">
              <bitcraft-inventory :inventory="playerInventory"></bitcraft-inventory>
            </v-col>

            <template v-if="!inventoryPending" v-for="(inventory, index) in inventorys">
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