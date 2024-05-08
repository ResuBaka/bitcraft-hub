<script setup lang="ts">
function entityIdToName(entityId: number): string {
    if (!leaderboard.value?.players) {
        return entityId.toString();
    }
    else {
        return leaderboard.value.players.find(p => p.entityID === entityId)?.entityName ?? entityId.toString();
    }
}

const { data: leaderboard, pending, error, refresh } = await useFetch('/api/bitcraft/leaderboard', {
    onResponse({ request, response, options }) {        
        //console.log(response);
    },
})

const topPlayers = computed(() => {
    return leaderboard?.value?.leaderboard ?? {}
});

const skills = computed(() => {
    return leaderboard?.value?.skills ?? []
});

const topPlayersByExp = computed(() => {
    return leaderboard?.value?.expTable ?? []
});

const topPlayersByLvl = computed(() => {
    return leaderboard?.value?.lvlTable ?? []
});

let selectedSkills = ref("Fishing");
const selectedCategory = computed(() => {
    return selectedSkills.value
});

</script>

<style lang="scss">
@use "~/assets/styles/leaderboard.scss";
</style>

<template>
    <v-layout class="justify-center" v-if="pending">
        <v-progress-circular indeterminate>
        </v-progress-circular>
    </v-layout>
    <template v-else-if="!pending">
        <div>
            <v-container class="mb-6 pa-0">
                <v-row align="start" no-gutters>
                    <v-col class="v-col-12 pa-0">
                        <div class="mb-2">
                            <v-sheet class="pa-2 ma-0">
                                <h1>Leaderboards</h1>
                            </v-sheet>
                        </div>
                        <div class="d-flex justify-space-between skill-buttons mb-3">
                            <v-btn :variant="'flat'" @click="selectedSkills = 'by_exp'" :active="selectedSkills === 'by_exp'">Total experience</v-btn>
                            <v-btn :variant="'flat'" @click="selectedSkills = 'by_level'" :active="selectedSkills === 'by_level'">Total level</v-btn>
                            <v-btn v-for="i in skills.slice(0,3)" :variant="'flat'" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
                        </div>
                        <div class="d-flex justify-space-between skill-buttons mb-3">
                            <v-btn v-for="i in skills.slice(3,8)" :variant="'flat'" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
                        </div>
                        <div class="d-flex justify-space-between skill-buttons mb-3">
                            <v-btn v-for="i in skills.slice(8,14)" :variant="'flat'" @click="selectedSkills = i.name" :active="selectedSkills === i.name">{{ i.name }}</v-btn>
                        </div>
                    </v-col>
                </v-row>
                <v-row v-if="(selectedSkills !== 'by_exp' && selectedSkills !== 'by_level')">
                    <v-col lass="v-col-12 pa-0">
                        <v-data-table density="compact" 
                        :items="topPlayers[selectedCategory]"  
                        :headers="[
                                    { title: 'Rank', value: 'rank' },
                                    { title: 'Player', value: 'player' },
                                    { title: 'Level', value: 'level' },
                                    { title: 'Experience', value: 'exp' }
                                ]" 
                        items-per-page="100">
                            <template v-slot:headers="{ columns }">
                                <tr>
                                    <template v-for="column in columns" :key="column.key">
                                        <th :class="{
                                            'text-left': column.key === 'rank',
                                            'text-center': column.key === 'player' || column.key === 'level',
                                            'text-right': column.key === 'exp'
                                        }">
                                            <span>{{ column.title }}</span>
                                        </th>
                                    </template>
                                </tr>
                            </template>
                            <template v-slot:item="{ item, index }">
                                <tr>
                                    <td># {{ index + 1 }}</td>
                                    <td class="text-center">
                                        <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                                            {{ entityIdToName(item.entity_id) }}
                                        </NuxtLink>
                                    </td>
                                    <td class="text-center">{{ item.experience_stacks[selectedSkills].level }}</td>
                                    <td class="text-right">{{ item.experience_stacks[selectedSkills].experience }}</td>
                                </tr>
                            </template>
                            <template #bottom></template>
                        </v-data-table>
                    </v-col>
                </v-row>
                <v-row v-if="selectedSkills === 'by_exp'">
                    <v-col lass="v-col-12 pa-0">
                        <v-data-table density="compact" 
                        :items="topPlayersByExp"  
                        :headers="[
                                    { title: 'Rank', value: 'rank' },
                                    { title: 'Player', value: 'player' },
                                    { title: 'Total Experience', value: 'exp' }
                                ]" 
                        items-per-page="100">
                            <template v-slot:headers="{ columns }">
                                <tr>
                                    <template v-for="column in columns" :key="column.key">
                                        <th :class="{
                                            'text-left': column.key === 'rank',
                                            'text-center': column.key === 'player',
                                            'text-right': column.key === 'exp'
                                        }">
                                            <span>{{ column.title }}</span>
                                        </th>
                                    </template>
                                </tr>
                            </template>
                            <template v-slot:item="{ item, index }">
                                <tr>
                                    <td># {{ index + 1 }}</td>
                                    <td class="text-center">
                                        <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                                            {{ entityIdToName(item.entity_id) }}
                                        </NuxtLink>
                                    </td>                                    
                                    <td class="text-right">{{ item.exp }}</td>
                                </tr>
                            </template>
                            <template #bottom></template>
                        </v-data-table>
                    </v-col>
                </v-row>
                <v-row v-if="selectedSkills === 'by_level'">
                    <v-col lass="v-col-12 pa-0">
                        <v-data-table density="compact" 
                        :items="topPlayersByLvl"  
                        :headers="[
                                    { title: 'Rank', value: 'rank' },
                                    { title: 'Player', value: 'player' },
                                    { title: 'Total Level', value: 'level' }
                                ]" 
                        items-per-page="100">
                            <template v-slot:headers="{ columns }">
                                <tr>
                                    <template v-for="column in columns" :key="column.key">
                                        <th :class="{
                                            'text-left': column.key === 'rank',
                                            'text-center': column.key === 'player',
                                            'text-right': column.key === 'level'
                                        }">
                                            <span>{{ column.title }}</span>
                                        </th>
                                    </template>
                                </tr>
                            </template>
                            <template v-slot:item="{ item, index }">
                                <tr>
                                    <td># {{ index + 1 }}</td>
                                    <td class="text-center">
                                        <NuxtLink :to="{ path: 'players/' + item.entity_id }">
                                            {{ entityIdToName(item.entity_id) }}
                                        </NuxtLink>
                                    </td>                                    
                                    <td class="text-right">{{ item.lvl }}</td>
                                </tr>
                            </template>
                            <template #bottom></template>
                        </v-data-table>
                    </v-col>
                </v-row>
            </v-container>
        </div>
    </template>
</template>