<script setup lang="ts">
const levelingData = [
    {
        level: 1,
        xp: 0,
    }, {
        level: 2,
        xp: 640,
    }, {
        level: 3,
        xp: 1330,
    }, {
        level: 4,
        xp: 2090,
    }, {
        level: 5,
        xp: 2920,
    }, {
        level: 6,
        xp: 3830,
    }, {
        level: 7,
        xp: 4820,
    }, {
        level: 8,
        xp: 5890,
    }, {
        level: 9,
        xp: 7070,
    }, {
        level: 10,
        xp: 8350,
    }, {
        level: 11,
        xp: 9740,
    }, {
        level: 12,
        xp: 11260,
    }, {
        level: 13,
        xp: 12920,
    }, {
        level: 14,
        xp: 14730,
    }, {
        level: 15,
        xp: 16710,
    }, {
        level: 16,
        xp: 18860,
    }, {
        level: 17,
        xp: 21210,
    }, {
        level: 18,
        xp: 23770,
    }, {
        level: 19,
        xp: 26560,
    }, {
        level: 20,
        xp: 29600,
    }, {
        level: 21,
        xp: 32920,
    }, {
        level: 22,
        xp: 36550,
    }, {
        level: 23,
        xp: 40490,
    }, {
        level: 24,
        xp: 44800,
    }, {
        level: 25,
        xp: 49490,
    }, {
        level: 26,
        xp: 54610,
    }, {
        level: 27,
        xp: 60200,
    }, {
        level: 28,
        xp: 66290,
    }, {
        level: 29,
        xp: 72930,
    }, {
        level: 30,
        xp: 80170,
    }, {
        level: 31,
        xp: 88060,
    }, {
        level: 32,
        xp: 96670,
    }, {
        level: 33,
        xp: 106060,
    }, {
        level: 34,
        xp: 116300,
    }, {
        level: 35,
        xp: 127470,
    }, {
        level: 36,
        xp: 139650,
    }, {
        level: 37,
        xp: 152930,
    }, {
        level: 38,
        xp: 167410,
    }, {
        level: 39,
        xp: 183200,
    }, {
        level: 40,
        xp: 200420,
    }, {
        level: 41,
        xp: 219200,
    }, {
        level: 42,
        xp: 239680,
    }, {
        level: 43,
        xp: 262020,
    }, {
        level: 44,
        xp: 286370,
    }, {
        level: 45,
        xp: 312930,
    }, {
        level: 46,
        xp: 341890,
    }, {
        level: 47,
        xp: 373480,
    }, {
        level: 48,
        xp: 407920,
    }, {
        level: 49,
        xp: 445480,
    }, {
        level: 50,
        xp: 486440,
    }, {
        level: 51,
        xp: 531110,
    }, {
        level: 52,
        xp: 579820,
    }, {
        level: 53,
        xp: 632940,
    }, {
        level: 54,
        xp: 690860,
    }, {
        level: 55,
        xp: 754030,
    }, {
        level: 56,
        xp: 822920,
    }, {
        level: 57,
        xp: 898040,
    }, {
        level: 58,
        xp: 979960,
    }, {
        level: 59,
        xp: 1069290,
    }, {
        level: 60,
        xp: 1166710,
    }, {
        level: 61,
        xp: 1272950,
    }, {
        level: 62,
        xp: 1388800,
    }, {
        level: 63,
        xp: 1515140,
    }, {
        level: 64,
        xp: 1652910,
    }, {
        level: 65,
        xp: 1803160,
    }, {
        level: 66,
        xp: 1967000,
    }, {
        level: 67,
        xp: 2145660,
    }, {
        level: 68,
        xp: 2340500,
    }, {
        level: 69,
        xp: 2552980,
    }, {
        level: 70,
        xp: 2784680,
    }, {
        level: 71,
        xp: 3037360,
    }, {
        level: 72,
        xp: 3312900,
    }, {
        level: 73,
        xp: 3613390,
    }, {
        level: 74,
        xp: 3941070,
    }, {
        level: 75,
        xp: 4298410,
    }, {
        level: 76,
        xp: 4688090,
    }, {
        level: 77,
        xp: 5113030,
    }, {
        level: 78,
        xp: 5576440,
    }, {
        level: 79,
        xp: 6081800,
    }, {
        level: 80,
        xp: 6632890,
    }, {
        level: 81,
        xp: 7233850,
    }, {
        level: 82,
        xp: 7889210,
    }, {
        level: 83,
        xp: 8603890,
    }, {
        level: 84,
        xp: 9383250,
    }, {
        level: 85,
        xp: 10233150,
    }, {
        level: 86,
        xp: 11159970,
    }, {
        level: 87,
        xp: 12170670,
    }, {
        level: 88,
        xp: 13272850,
    }, {
        level: 89,
        xp: 14474790,
    }, {
        level: 90,
        xp: 15785510,
    }, {
        level: 91,
        xp: 17214860,
    }, {
        level: 92,
        xp: 18773580,
    }, {
        level: 93,
        xp: 20473370,
    }, {
        level: 94,
        xp: 22327010,
    }, {
        level: 95,
        xp: 24348420,
    }, {
        level: 96,
        xp: 26552780,
    }, {
        level: 97,
        xp: 28956650,
    }, {
        level: 98,
        xp: 31578090,
    }, {
        level: 99,
        xp: 34436800,
    }, {
        level: 100,
        xp: 37554230,
    }
]
function XPToLevel(xp: number) {
    for (const data of levelingData) {
        if (xp < data.xp) {
            return data.level - 1
        }
    }
    return 100
}

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
                                    <td class="text-center">{{ XPToLevel(item.experience_stacks[skills.find(s => s.name === selectedSkills)?.id ?? 0]) }}</td>
                                    <td class="text-right">{{ item.experience_stacks[skills.find(s => s.name === selectedSkills)?.id ?? 0] }}</td>
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