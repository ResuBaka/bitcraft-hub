<script setup lang="ts">
const numberFormat = new Intl.NumberFormat(undefined);

const { data: leaderboard, pending } = await useFetch(
  "/api/bitcraft/leaderboard",
  {
    lazy: true,
  },
);

const skills = computed(() => {
  if (!leaderboard.value) {
    return [];
  }

  return Object.keys(leaderboard.value).filter((name) => {
    return name !== "Experience" && name !== "Level";
  });
});

let selectedSkills = ref("Fishing");

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
</script>

<template>
  <v-container class="fill-height" v-if="pending">
    <v-row align-content="center" justify="center" class="fill-height">
      <v-progress-circular indeterminate></v-progress-circular>
    </v-row>
  </v-container>
  <v-container fluid v-else-if="!pending">
    <v-row align="start">
      <v-col class="v-col-12">
        <div class="mb-2">
          <v-sheet class="text-center text-md-left text-xl-center">
            <h1 class="pl-md-3 pl-xl-0">Leaderboards</h1>
          </v-sheet>
        </div>
      </v-col>
      <v-col v-for="skill in skillMenu" :key="skill.key"
             :style="$vuetify.display.lgAndUp ? ' flex: 1 0 18%;' : ''"
             cols="12"
             sm="4"
      >
        <v-btn variant="flat" block @click="selectedSkills = skill.key" :active="selectedSkills === skill.key">
          {{ skill.text }}
        </v-btn>
      </v-col>
    </v-row>
    <v-row v-if="selectedSkills !== 'Experience' && selectedSkills !== 'Level'">
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
