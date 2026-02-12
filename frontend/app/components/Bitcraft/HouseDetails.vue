<script setup lang="ts">
import type { HouseResponse } from "~/types/HouseResponse";
import type { HouseInventoriesResponse } from "~/types/HouseInventoriesResponse";
import type { InventoryChangelog } from "~/types/InventoryChangelog";

const props = defineProps<{
  house: HouseResponse;
}>();

const tab = ref(null);

const {
  data: inventories,
  pending: invPending,
} = await useLazyFetchMsPack<HouseInventoriesResponse>(
  () => `/api/bitcraft/houses/${props.house.entity_id}/inventories`,
);

const {
  data: changelog,
  pending: changelogPending,
} = await useLazyFetchMsPack<InventoryChangelog[]>(
  () => `/api/bitcraft/inventorys/changes/${props.house.entity_id}`,
);

const theme = useTheme();
const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

const getRankName = (rank: number) => {
  switch (rank) {
    case 7:
      return "Owner";
    case 6:
      return "Admin";
    case 5:
      return "Resident";
    case 1:
      return "Guest";
    default:
      return `Rank ${rank}`;
  }
};
</script>

<template>
  <v-card variant="outlined" class="mb-4">
    <v-card-title class="d-flex align-center gap-4">
      <span class="text-h6 font-weight-black">House {{ house.entity_id }}</span>
      <v-chip color="primary" label size="small">{{ house.region }}</v-chip>
      <v-spacer></v-spacer>
      <v-chip :color="house.is_empty ? 'grey' : 'success'" size="small" label>
        {{ house.is_empty ? 'Empty' : 'Occupied' }}
      </v-chip>
    </v-card-title>

    <v-tabs v-model="tab" bg-color="transparent" density="compact">
      <v-tab value="overview">Overview</v-tab>
      <v-tab value="permissions">Permissions ({{ house.permissions.length }})</v-tab>
      <v-tab value="inventories">Inventories ({{ inventories?.inventories.length || 0 }})</v-tab>
      <v-tab value="changelog">Changes ({{ changelog?.length || 0 }})</v-tab>
    </v-tabs>

    <v-divider></v-divider>

    <v-card-text>
      <v-window v-model="tab">
        <!-- Overview -->
        <v-window-item value="overview">
          <v-row dense>
            <v-col cols="12" md="6">
              <v-list lines="two" :class="computedClass" class="rounded" density="compact">
                <v-list-item title="Entity ID" :subtitle="house.entity_id.toString()"></v-list-item>
                <v-divider></v-divider>
                <v-list-item title="Entrance Building ID" :subtitle="house.entrance_building_entity_id.toString()"></v-list-item>
              </v-list>
            </v-col>
            <v-col cols="12" md="6">
              <v-list lines="two" :class="computedClass" class="rounded" density="compact">
                <v-list-item title="Region" :subtitle="`${house.region} (Index: ${house.region_index})`"></v-list-item>
                <v-divider></v-divider>
                <v-list-item title="Rank" :subtitle="house.rank.toString()"></v-list-item>
              </v-list>
            </v-col>
          </v-row>
        </v-window-item>

        <!-- Permissions -->
        <v-window-item value="permissions">
          <v-table v-if="house.permissions.length > 0" density="compact">
            <thead>
              <tr>
                <th class="text-left">Player ID</th>
                <th class="text-left">Username</th>
                <th class="text-left">Group</th>
                <th class="text-left">Rank</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="p in house.permissions" :key="p.allowed_entity_id.toString()">
                <td>
                  <nuxt-link :to="{ name: 'players-id', params: { id: p.allowed_entity_id.toString() } }">
                    {{ p.allowed_entity_id.toString() }}
                  </nuxt-link>
                </td>
                <td>{{ p.allowed_username || 'Unknown' }}</td>
                <td>{{ p.group }}</td>
                <td>
                  <v-chip :color="p.rank === 7 ? 'gold' : 'blue-grey'" size="x-small">
                    {{ getRankName(p.rank) }}
                  </v-chip>
                </td>
              </tr>
            </tbody>
          </v-table>
          <div v-else class="text-center py-4 text-grey text-caption">
            No explicit permissions found.
          </div>
        </v-window-item>

        <!-- Inventories -->
        <v-window-item value="inventories">
          <div v-if="invPending" class="text-center py-4">
            <v-progress-circular indeterminate size="24"></v-progress-circular>
          </div>
          <template v-else-if="inventories && inventories.inventories.length > 0">
            <bitcraft-inventory
              v-for="inv in inventories.inventories"
              :key="inv.entity_id.toString()"
              :inventory="inv"
              class="mb-2"
            />
          </template>
          <div v-else class="text-center py-4 text-grey text-caption">
            No interior inventories found.
          </div>
        </v-window-item>

        <!-- Changelog -->
        <v-window-item value="changelog">
          <div v-if="changelogPending" class="text-center py-4">
            <v-progress-circular indeterminate size="24"></v-progress-circular>
          </div>
          <template v-else-if="changelog && changelog.length > 0">
            <bitcraft-inventory-changes :items="changelog" />
          </template>
          <div v-else class="text-center py-4 text-grey text-caption">
            No inventory changes found.
          </div>
        </v-window-item>
      </v-window>
    </v-card-text>
  </v-card>
</template>

<style scoped>
.gap-4 {
  gap: 16px;
}
</style>
