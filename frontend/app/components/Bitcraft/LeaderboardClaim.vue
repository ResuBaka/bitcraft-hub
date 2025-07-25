<script setup lang="ts">
import {useNow} from "@vueuse/core";

const numberFormat = new Intl.NumberFormat(undefined);
const props = defineProps<{
  claimId: number | bigint;
}>();
const leaderboard_collapsible = ref([]);

const { data: leaderboard, pending } = await useFetchMsPack(
  () => {
    return `/api/bitcraft/leaderboard/claims/${props["claimId"]}`;
  },
  {
    lazy: true,
  },
);

const skills = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return [];
  }

  return Object.keys(leaderboard.value.leaderboard).filter((name) => {
    return name !== "Experience" && name !== "Level";
  });
});

let selectedSkills = ref("Experience");

let skillMenu = computed(() => {
  const skillMenu = [
    { key: "Experience", text: "Total experience" },
    { key: "Level", text: "Total level" },
  ];

  for (const skill of skills.value) {
    skillMenu.push({
      key: skill,
      text: skill,
    });
  }

  return skillMenu;
});

const totelExperiencePerHourAverage = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return 0;
  }

  let totalExperience = 0;

  totalExperience += leaderboard.value.leaderboard["Experience"].reduce(
    (acc, curr) => {
      return (
        acc +
        Math.ceil(
          curr.experience /
            Math.ceil(leaderboard?.value?.player_map[curr.player_id] / 3600),
        )
      );
    },
    0,
  );

  return Math.ceil(
    totalExperience / leaderboard.value.leaderboard["Experience"].length,
  );
});

const icons = {
  Fishing: { icon: "mdi-fish", color: "blue" },
  Mining: { icon: "mdi-pickaxe", color: "grey" },
  Woodcutting: { icon: "mdi-forest", color: "green" },
  Farming: { icon: "mdi-sprout", color: "green" },
  Carpentry: { icon: "mdi-hand-saw", color: "brown" },
  Foraging: { icon: "mdi-leaf", color: "green" },
  Forestry: { icon: "mdi-axe", color: "brown" },
  Masonry: { icon: "mdi-screwdriver", color: "grey" },
  Smithing: { icon: "mdi-anvil", color: "grey" },
  Scholar: { icon: "mdi-school", color: "" },
  Hunting: { icon: "mdi-bow-arrow", color: "" },
  Cooking: { icon: "mdi-stove", color: "" },
  // Leatherworking: { icon: "", color: "" },
  // Tailoring: { icon: "", color: "" },
  // Experience: { icon: "", color: "" },
  // Level: { icon: "", color: "" },
};

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

const now = useNow({ interval: 1000, controls: true });
const game_start = new Date('2025-06-21T13:00:05Z');

const countDownUntilResearchIsFinished = computed(() => {
  const diff = now.now.value.getTime() - game_start.getTime();

  return {
    days: Math.floor(diff / (1000 * 60 * 60 * 24)),
    hours: Math.floor((diff / (1000 * 60 * 60)) % 24),
    minutes: Math.floor((diff / 1000 / 60) % 60),
    seconds: Math.floor((diff / 1000) % 60),
  };
});
</script>

<template>
  <v-layout class="justify-center" v-if="pending">
    <v-progress-circular indeterminate></v-progress-circular>
  </v-layout>
  <template fluid v-else-if="!pending">
    <v-expansion-panels v-model="leaderboard_collapsible">
      <v-expansion-panel value="leaderboard">
        <v-expansion-panel-title>
          <v-row>
            <v-col class="d-flex justify-center">
              <h2 class="pl-md-3 pl-xl-0">Leaderboards</h2>
            </v-col>
          </v-row>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <v-row dense align="start">

            <v-col v-if="$vuetify.display.xs">
              <v-select v-model="selectedSkills" item-value="key" item-title="text" :items="skillMenu" label="Skills"
                        outlined
                        dense center-affix>
                <template #item="{ props, item }">
                  <v-list-item v-bind="props" class="text-center">
                    <template #append v-if="icons[item.value]">
                      <v-icon :color="icons[item.value].color">{{ icons[item.value].icon }}</v-icon>
                    </template>
                    <template #prepend v-if="icons[item.value]">
                      <v-icon :color="icons[item.value].color">{{ icons[item.value].icon }}</v-icon>
                    </template>
                  </v-list-item>
                </template>
                <template #selection="{ item }">
                  <v-list-item class="w-100 text-center">
                    <template #append v-if="icons[item.value]">
                      <v-icon :color="icons[item.value].color">{{ icons[item.value].icon }}</v-icon>
                    </template>
                    <template #prepend v-if="icons[item.value]">
                      <v-icon :color="icons[item.value].color">{{ icons[item.value].icon }}</v-icon>
                    </template>
                    <v-list-item-title>{{ item.title }}</v-list-item-title>
                  </v-list-item>
                </template>
              </v-select>
            </v-col>
            <v-col v-else v-for="skill in skillMenu" :key="skill.key"
                   :style="$vuetify.display.lgAndUp ? ' flex: 1 0 18%;' : ''"
                   cols="12"
                   sm="4"
            >
              <v-btn variant="flat" block @click="selectedSkills = skill.key" :active="selectedSkills === skill.key">
                <template #prepend v-if="icons[skill.key]">
                  <v-icon :color="icons[skill.key].color">{{ icons[skill.key].icon }}</v-icon>
                </template>
                {{ skill.text }}
                <template #append v-if="icons[skill.key]">
                  <v-icon :color="icons[skill.key].color">{{ icons[skill.key].icon }}</v-icon>
                </template>
              </v-btn>
            </v-col>
          </v-row>
          <v-row v-if="selectedSkills !== 'Experience' && selectedSkills !== 'Level' && selectedSkills !== 'Time Played' && selectedSkills !== 'Time Online'">
            <v-col lass="v-col-12 pa-0">
              <v-table hover>
                <thead>
                <tr>
                  <th>Rank</th>
                  <th class="text-center">Player</th>
                  <th class="text-center">level</th>
                  <th class="text-end">Experience</th>
                </tr>
                </thead>
                <tbody>
                <tr v-for="(item, index) in leaderboard.leaderboard[selectedSkills]" :key="item.player_id">
                  <td>{{ index + 1 }}</td>
                  <td class="text-center">
                    <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                              :to="{ name: 'players-id', params: { id: item.player_id } }">
                      {{ item.player_name }}
                    </NuxtLink>
                  </td>
                  <td class="text-center">{{ item.level }}</td>
                  <td class="text-end">{{ numberFormat.format(item.experience) }}</td>
                </tr>
                </tbody>
              </v-table>
            </v-col>
          </v-row>
          <v-row v-if="selectedSkills === 'Experience'">
            <v-col lass="v-col-12 pa-0">
              <v-table hover>
                <thead>
                <tr>
                  <th>Rank</th>
                  <th class="text-center">Player</th>
                  <th class="text-end">Experience/h ({{ numberFormat.format(totelExperiencePerHourAverage) }})</th>
                  <th class="text-end">Experience</th>
                </tr>
                </thead>
                <tbody>
                <tr v-for="(item, index) in leaderboard.leaderboard[selectedSkills]" :key="item.player_id">
                  <td>{{ index + 1 }}</td>
                  <td class="text-center">
                    <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                              :to="{ name: 'players-id', params: { id: item.player_id } }">
                      {{ item.player_name }}
                    </NuxtLink>
                  </td>
                  <td class="text-end">{{ numberFormat.format(Math.ceil(item.experience / Math.ceil(leaderboard.player_map[item.player_id] / 3600))) }}</td>
                  <td class="text-end">{{ numberFormat.format(item.experience) }}</td>
                </tr>
                </tbody>
              </v-table>
            </v-col>
          </v-row>
          <v-row v-if="selectedSkills === 'Time Played' || selectedSkills === 'Time Online'">
            <v-col lass="v-col-12 pa-0">
              <v-table hover>
                <thead>
                <tr>
                  <th>Rank</th>
                  <th class="text-center">Player</th>
                  <th class="text-end">Time (Game is online since <strong v-if="countDownUntilResearchIsFinished.days">{{ countDownUntilResearchIsFinished.days }}d </strong><strong v-if="countDownUntilResearchIsFinished.hours">{{ countDownUntilResearchIsFinished.hours }}h </strong><strong v-if="countDownUntilResearchIsFinished.minutes">{{ countDownUntilResearchIsFinished.minutes }}m </strong><strong v-if="countDownUntilResearchIsFinished.seconds">{{ countDownUntilResearchIsFinished.seconds }}s</strong>)</th>
                </tr>
                </thead>
                <tbody>
                <tr v-for="(item, index) in leaderboard.leaderboard[selectedSkills]" :key="item.player_id">
                  <td>{{ index + 1 }}</td>
                  <td class="text-center">
                    <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                              :to="{ path: 'players/' + item.player_id }">
                      {{ item.player_name }}
                    </NuxtLink>
                  </td>
                  <td class="text-end">{{ secondsToDaysMinutesSecondsFormat(item.time_played) }}</td>
                </tr>
                </tbody>
              </v-table>
            </v-col>
          </v-row>
          <v-row v-if="selectedSkills === 'Level'">
            <v-col lass="v-col-12 pa-0">
              <v-table hover>
                <thead>
                <tr>
                  <th>Rank</th>
                  <th class="text-center">Player</th>
                  <th class="text-end">Level</th>
                </tr>
                </thead>
                <tbody>
                <tr v-for="(item, index) in leaderboard.leaderboard[selectedSkills]" :key="item.player_id">
                  <td>{{ index + 1 }}</td>
                  <td class="text-center">
                    <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                              :to="{ name: 'players-id', params: { id: item.player_id } }">
                      {{ item.player_name }}
                    </NuxtLink>
                  </td>
                  <td class="text-end">{{ numberFormat.format(item.level) }}</td>
                </tr>
                </tbody>
              </v-table>
            </v-col>
          </v-row>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
  </template>
</template>

<style scoped>
:deep(.v-select__selection) {
  width: 100%;
  justify-content: center;
}
</style>