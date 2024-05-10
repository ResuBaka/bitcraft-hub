<script setup lang="ts">
const numberFormat = new Intl.NumberFormat(undefined);

const {
  data: leaderboard, pending, error, refresh, } = await useFetch("/api/bitcraft/leaderboard");

const topPlayers = computed(() => {
  return leaderboard?.value?.leaderboard ?? {};
});

const skills = computed(() => {
  return leaderboard?.value?.skills ?? [];
});

const topPlayersByExp = computed(() => {
  return leaderboard?.value?.expTable ?? [];
});

const topPlayersByLvl = computed(() => {
  return leaderboard?.value?.lvlTable ?? [];
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
              <v-btn variant="flat" @click="selectedSkills = 'by_exp'" :active="selectedSkills === 'by_exp'" >Total experience</v-btn>
              <v-btn variant="flat" @click="selectedSkills = 'by_level'" :active="selectedSkills === 'by_level'" >Total level</v-btn>
              <v-btn v-for="i in skills.slice(0, 3)" variant="flat" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
            </div>
            <div class="d-flex justify-center justify-md-space-between flex-wrap skill-buttons mb-0 mb-md-3">
              <v-btn v-for="i in skills.slice(3, 8)" variant="flat" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
            </div>
            <div class="d-flex justify-center justify-md-space-between flex-wrap skill-buttons mb-0 mb-md-3">
              <v-btn v-for="i in skills.slice(8, 14)" variant="flat" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
            </div>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills !== 'by_exp' && selectedSkills !== 'by_level'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table 
              density="compact"
              :items="topPlayers[selectedSkills]"
              :headers="[
              { title: 'Rank', value: 'rank', align: 'start' },
              { title: 'Player', value: 'player', align: 'center' },
              { title: 'Level', value: 'level', align: 'center' },
              { title: 'Experience', value: 'exp', align: 'end'}
              ]"
              hover
              items-per-page="100">
              <template v-slot:item.rank="{ index }">
                # {{ index + 1 }}
              </template>
              <template v-slot:item.level="{ item }">
                {{ item.experience_stacks[selectedSkills].level }}
              </template>
              <template v-slot:item.player="{ item }">
                <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                  {{ item.entity_name }}
                </NuxtLink>
              </template>
              <template v-slot:item.exp="{ item }">
                {{ numberFormat.format(item.experience_stacks[selectedSkills].experience) }}
              </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills === 'by_exp'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table
              density="compact"
              :items=topPlayersByExp
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
                  <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                    {{ item.entity_name }}
                  </NuxtLink>
                </template>
                <template v-slot:item.exp="{ item }">
                  {{  numberFormat.format(item.exp) }}
                </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
        <v-row v-if="selectedSkills === 'by_level'">
          <v-col lass="v-col-12 pa-0">
            <v-data-table
              density="compact"
              :items=topPlayersByLvl
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
                  <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                    {{ item.entity_name }}
                  </NuxtLink>
                </template>
                <template v-slot:item.level="{ item }">
                  {{  numberFormat.format(item.lvl) }}
                </template>
              <template #bottom></template>
            </v-data-table>
          </v-col>
        </v-row>
      </v-container>
    </div>
  </template>
</template>
