<script setup lang="ts">
const numberFormat = new Intl.NumberFormat(undefined);
const props = defineProps<{
  claimId: number;
}>();

const {
  data: leaderboard,
  pending,
  error,
  refresh,
} = await useFetch(`/api/bitcraft/leaderboard/claims/${props["claimId"]}`, {
  lazy: true,
});

const skills = computed(() => {
  if (!leaderboard.value) {
    return [];
  }

  return Object.keys(leaderboard.value).filter((name) => {
    return name !== "Experience" && name !== "Level";
  });
});

let selectedSkills = ref("Fishing");
</script>

<style scoped lang="scss">
.skill-buttons {
  button {
    width: 14rem;
  }
}
</style>

<template>
  <v-layout class="justify-center" v-if="pending">
    <v-progress-circular indeterminate> </v-progress-circular>
  </v-layout>
  <template v-else-if="!pending">
    <div>
      <v-container class="mb-6 pa-0">
        <v-row align="start" no-gutters>
          <v-col class="v-col-12 pa-0">
            <div class="mb-2">
              <v-sheet class="pa-2 ma-0 text-center text-md-left text-xl-center">
                <h1>Leaderboards</h1>
              </v-sheet>
            </div>
            <div class="d-flex justify-center justify-md-space-between flex-wrap skill-buttons mb-0 mb-md-3">
              <v-btn variant="flat" @click="selectedSkills = 'Experience'" :active="selectedSkills === 'Experience'" >Total experience</v-btn>
              <v-btn variant="flat" @click="selectedSkills = 'Level'" :active="selectedSkills === 'Level'" >Total level</v-btn>
              <v-btn v-for="name in skills.slice(0, 3)" variant="flat" @click="selectedSkills = name" :active="selectedSkills === name">{{ name }}</v-btn>
            </div>
            <div class="d-flex justify-center justify-md-space-between flex-wrap skill-buttons mb-0 mb-md-3">
              <v-btn v-for="name in skills.slice(3, 8)" variant="flat" @click="selectedSkills = name" :active="selectedSkills === name">{{ name }}</v-btn>
            </div>
            <div class="d-flex justify-center justify-md-space-between flex-wrap skill-buttons mb-0 mb-md-3">
              <v-btn v-for="name in skills.slice(8, 14)" variant="flat" @click="selectedSkills = name" :active="selectedSkills === name">{{ name }}</v-btn>
            </div>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills !== 'Experience' && selectedSkills !== 'Level'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table 
              density="compact"
              :items="leaderboard[selectedSkills]"
              :headers="[
              { title: 'Rank', value: 'rank', align: 'start' },
              { title: 'Player', value: 'player_name', align: 'center' },
              { title: 'Level', value: 'level', align: 'center' },
              { title: 'Experience', value: 'exp', align: 'end'}
              ]"
              hover
              items-per-page="100">
              <template v-slot:item.rank="{ index }">
                # {{ index + 1 }}
              </template>
              <template v-slot:item.level="{ value }">
                {{ value }}
              </template>
              <template v-slot:item.player_name="{ item }">
                <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'players-id', parameter: { id: item.player_id } }">
                  {{ item.player_name }}
                </NuxtLink>
              </template>
              <template v-slot:item.exp="{ item }">
                {{ numberFormat.format(item.experience) }}
              </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills === 'Experience'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table
              density="compact"
              :items="leaderboard[selectedSkills]"
              :headers="[
                { title: 'Rank', value: 'rank', align: 'start' },
                { title: 'Player', value: 'player', align: 'center' },                
                { title: 'Experience', value: 'exp', align: 'end' },
              ]"
              hover
              items-per-page="100">
                <template v-slot:item.rank="{ index }">
                  # {{ index + 1 }}
                </template>
                <template v-slot:item.player="{ item }">
                  <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'players-id', parameter: { id: item.player_id } }">
                    {{ item.player_name }}
                  </NuxtLink>
                </template>
                <template v-slot:item.exp="{ item }">
                  {{ numberFormat.format(item.experience) }}
                </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills === 'Level'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table
              density="compact"
              :items="leaderboard[selectedSkills]"
              :headers="[
                { title: 'Rank', value: 'rank', align: 'start' },
                { title: 'Player', value: 'player', align: 'center' },                
                { title: 'Level', value: 'level', align: 'end' },
              ]"
              hover
              items-per-page="100">
                <template v-slot:item.rank="{ index }">
                  # {{ index + 1 }}
                </template>
                <template v-slot:item.player="{ item }">
                  <NuxtLink class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'players-id', parameter: { id: item.player_id } }">
                    {{ item.player_name }}
                  </NuxtLink>
                </template>
                <template v-slot:item.level="{ item }">
                  {{  numberFormat.format(item.level) }}
                </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
      </v-container>
    </div>
  </template>
</template>
