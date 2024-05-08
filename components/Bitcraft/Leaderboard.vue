<script setup lang="ts">

//const theme = useTheme();

const backgroundColorRow = ({ index }) => {
    return {
        class: index % 2 === 0 ? "" : "bg-surface-light",
    };
};

const headers = [
    { title: 'Rank', value: 'rank' },
    { title: 'Player', value: 'player' },
    { title: 'Level', value: 'level' },
    { title: 'Experience', value: 'exp' },
]

const players = computed(() => {
    const players: { rank: number, name: string, level: number, exp: number } = [];
    for (let i = 0; i < 100; i++) {

        players.push({
            rank: i + 1,
            player: "Voxel",
            level: 100,
            exp: 4899711 + i
        })
    }

    console.log("1", players);
    return (
        players
    );
});
</script>

<style lang="scss">
@use "~/assets/styles/leaderboard.scss";
</style>

<template>
    <div>
        <v-container class="mb-6 pa-0">
            <v-row align="start" no-gutters>
                <v-col lass="v-col-12 pa-0">
                    <div class="mb-2">
                        <v-sheet class="pa-2 ma-0">
                            <h1>Leaderboards</h1>
                        </v-sheet>
                    </div>
                    <div class="d-flex justify-space-between skill-buttons mb-3">
                        <v-btn>Total experience</v-btn>
                        <v-btn>Total level</v-btn>
                        <v-btn>Farming</v-btn>
                        <v-btn>Fishing</v-btn>
                        <v-btn>Tailoring</v-btn>
                    </div>
                    <div class="d-flex justify-space-between skill-buttons mb-3">
                        <v-btn>Forestry</v-btn>
                        <v-btn>Foraging</v-btn>
                        <v-btn>Masonry</v-btn>
                        <v-btn>Smithing</v-btn>
                        <v-btn>Scholar</v-btn>
                    </div>
                    <div class="d-flex justify-space-between skill-buttons mb-3">
                        <v-btn>Hunting</v-btn>
                        <v-btn>Mining</v-btn>
                        <v-btn>Carpentry</v-btn>
                        <v-btn>Cooking</v-btn>
                        <v-btn>Leatherworking</v-btn>
                    </div>
                </v-col>

            </v-row>
            <v-row>
                <v-col lass="v-col-12 pa-0">
                    <v-data-table density="compact" :items="players" :headers="headers" items-per-page="100">
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
                        <template v-slot:item="{ item }">
                            <tr>
                                <td># {{ item.rank }}</td>
                                <td class="text-center">{{ item.player }}</td>
                                <td class="text-center">{{ item.level }}</td>
                                <td class="text-right">{{ item.exp }}</td>
                            </tr>
                        </template>
                        <template #bottom></template>
                    </v-data-table>
                </v-col>
            </v-row>
        </v-container>
    </div>
</template>