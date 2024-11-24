<script setup lang="ts">
const numberFormat = new Intl.NumberFormat(undefined);
const {
  public: { api },
} = useRuntimeConfig();

const {
  data: leaderboard,
  pending,
  refresh,
} = await useFetch(
  () => {
    return `${api.base}/leaderboard`;
  },
  {
    lazy: true,
  },
);

const route = useRoute();

const skills = computed(() => {
  if (!leaderboard.value) {
    return [];
  }

  return Object.keys(leaderboard.value).filter((name) => {
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

const enableRefresh = ref(false);

const refreshTimer = ref<NodeJS.Timeout | null>(null);

const times = 10000 / 100;
const untilRefresh = ref(0);

const toggleRefresh = () => {
  enableRefresh.value = !enableRefresh.value;

  if (enableRefresh.value) {
    refreshTimer.value = setInterval(() => {
      if (untilRefresh.value >= times) {
        refresh();
        untilRefresh.value = 0;
      } else {
        untilRefresh.value++;
      }
    }, 100);
  } else {
    untilRefresh.value = 0;
    if (refreshTimer.value) {
      clearInterval(refreshTimer.value);
    }
  }
};

const queryRefresh = route.query?.refresh ?? false;

if (queryRefresh) {
  toggleRefresh();
}

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
</script>

<template>
  <v-container class="fill-height" v-if="pending">
    <v-row align-content="center" justify="center" class="fill-height">
      <v-progress-circular indeterminate></v-progress-circular>
    </v-row>
  </v-container>
  <v-container fluid v-else-if="!pending">
    <v-row dense align="start">
      <v-col class="v-col-12">
        <div>
          <v-sheet class="text-center">
            <v-progress-linear
                v-if="enableRefresh"
                v-model="untilRefresh"
                color="blue"
                height="40"
                :max="times"
                @click="toggleRefresh"
            >
              <strong>Leaderboards(Auto-refresh)</strong>
            </v-progress-linear>
            <h1 v-else class="pl-md-3 pl-xl-0" @click="toggleRefresh">Leaderboards</h1>

          </v-sheet>
        </div>
      </v-col>
      <v-col v-if="$vuetify.display.xs">
        <v-select v-model="selectedSkills" item-value="key" item-title="text" :items="skillMenu" label="Skills" outlined
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
    <v-row v-if="selectedSkills !== 'Experience' && selectedSkills !== 'Level'">
      <v-col lass="v-col-12 pa-0">
        <v-table density="compact" hover>
          <thead>
          <tr>
            <th>Rank</th>
            <th class="text-center">Player</th>
            <th class="text-center">level</th>
            <th class="text-end">Experience</th>
          </tr>
          </thead>
          <tbody>
          <tr v-for="(item, index) in leaderboard[selectedSkills]" :key="item.player_id">
            <td>{{ index + 1 }}</td>
            <td class="text-center">
              <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                        :to="{ path: 'players/' + item.player_id }">
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
            <th class="text-end">Experience</th>
          </tr>
          </thead>
          <tbody>
          <tr v-for="(item, index) in leaderboard[selectedSkills]" :key="item.player_id">
            <td>{{ index + 1 }}</td>
            <td class="text-center">
              <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                        :to="{ path: 'players/' + item.player_id }">
                {{ item.player_name }}
              </NuxtLink>
            </td>
            <td class="text-end">{{ numberFormat.format(item.experience) }}</td>
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
          <tr v-for="(item, index) in leaderboard[selectedSkills]" :key="item.player_id">
            <td>{{ index + 1 }}</td>
            <td class="text-center">
              <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black"
                        :to="{ path: 'players/' + item.player_id }">
                {{ item.player_name }}
              </NuxtLink>
            </td>
            <td class="text-end">{{ numberFormat.format(item.level) }}</td>
          </tr>
          </tbody>
        </v-table>
      </v-col>
    </v-row>
  </v-container>
</template>

<style scoped>
:deep(.v-select__selection) {
  width: 100%;
  justify-content: center;
}
</style>
