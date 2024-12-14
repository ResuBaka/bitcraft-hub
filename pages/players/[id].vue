<script setup lang="ts">

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

registerWebsocketMessageHandler(
  "Experience",
  `experience.${route.params.id}`,
  (message) => {
    if (experienceFetch.value && experienceFetch.value[message.c.skill_name]) {
      let currentExperience =
        experienceFetch.value[message.c.skill_name].experience;
      experienceFetch.value[message.c.skill_name] = {
        ...experienceFetch.value[message.c.skill_name],
        experience: message.c.experience,
        level: message.c.level,
      };

      if (experienceFetch.value["Experience"]) {
        let newExperience = message.c.experience;
        let increase = newExperience - currentExperience;

        experienceFetch.value["Experience"] = {
          ...experienceFetch.value["Experience"],
          experience: experienceFetch.value["Experience"].experience + increase,
        };
      }
    }
  },
);

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
  return inventoryFetch.value?.inventorys ?? [];
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
</script>

<template>
  <v-container fluid>
    <v-layout class="justify-center" v-if="playerPnding">
      <v-progress-circular indeterminate>
      </v-progress-circular>
    </v-layout>
    <template v-else-if="player">
      <v-banner class="text-high-emphasis font-weight-black">Player: {{ player?.username }}</v-banner>
      <v-card>
        <v-card-text :class="computedClass">
          <v-table :class="computedClass" density="compact">
            <tbody>
            <tr style='text-align: right'>
              <th>signed_in:</th>
              <td>{{ player.signed_in }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>sign_in_timestamp:</th>
              <td>{{ player.sign_in_timestamp }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>session_start_timestamp:</th>
              <td>{{ player.session_start_timestamp }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>time_played:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(player.time_played) }}</td>
            </tr>
            <tr style='text-align: right'>
              <th>time_signed_in:</th>
              <td>{{ secondsToDaysMinutesSecondsFormat(player.time_signed_in) }}</td>
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
                    <v-col cols="12" md="4" lg="2" v-for="[skill,xp_info] of Object.entries(expeirence)" :key="skill">
                      <v-list :class="xp_info.classes.container">
                        <div :class="xp_info.classes.list"></div>
                        <v-list-item :class="xp_info.classes.content">
                          <v-list-item-title>{{ skill }}</v-list-item-title>
<!--                          {{ xp_info.experience ? numberFormat.format(xp_info.experience) : "" }}-->
                          <v-list-item-subtitle>Experience: <bitcraft-animated-number :value="xp_info.experience" :speed="8" :formater="numberFormat.format"></bitcraft-animated-number></v-list-item-subtitle>
                          <v-list-item-subtitle>Level: <bitcraft-animated-number v-if="xp_info.level" :value="xp_info.level" :speed="50"></bitcraft-animated-number></v-list-item-subtitle>
                          <v-list-item-subtitle>Rank: #<bitcraft-animated-number :value="xp_info.rank" :speed="50"></bitcraft-animated-number></v-list-item-subtitle>
                        </v-list-item>
                      </v-list>
                    </v-col>
                  </v-row>
                </v-card-text>
              </v-card>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>

      <v-card variant="text" v-if="inventorys.length">
        <v-card-title>Inventory's</v-card-title>
        <v-card-text>
          <v-row>
            <template v-if="!inventoryPending" v-for="(inventory, index) in inventorys">
              <v-col cols="12" md="6">
                <bitcraft-player-tool-belt v-if="inventory.nickname === 'Tool belt'" :inventory="inventory"></bitcraft-player-tool-belt>
                <bitcraft-inventory v-else-if="inventory.nickname === 'Wallet'" :inventory="inventory"></bitcraft-inventory>
                <bitcraft-inventory v-else-if="inventory.nickname === 'Inventory'" :inventory="inventory"></bitcraft-inventory>
                <bitcraft-inventory v-else :inventory="inventory"></bitcraft-inventory>
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