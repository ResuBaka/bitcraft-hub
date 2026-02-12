<script setup lang="ts">
import type { HouseResponse } from "~/types/HouseResponse";
import type { HouseInventoriesResponse } from "~/types/HouseInventoriesResponse";
import type { InventoryChangelog } from "~/types/InventoryChangelog";

const route = useRoute();
const id = route.params.id as string;

const tab = ref(null);

const { data: house, pending: housePending } =
  await useLazyFetchMsPack<HouseResponse>(() => `/api/bitcraft/houses/${id}`);

const {
  data: inventories,
  pending: invPending,
  refresh: refreshInventories,
} = await useLazyFetchMsPack<HouseInventoriesResponse>(
  () => `/api/bitcraft/houses/${id}/inventories`,
);

const {
  data: changelog,
  pending: changelogPending,
  refresh: refreshChangelog,
} = await useLazyFetchMsPack<InventoryChangelog[]>(
  () => `/api/bitcraft/inventorys/changes/${id}`,
);

const theme = useTheme();
const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

useSeoMeta({
  title: () =>
    house.value
      ? `${house.value.owner_username || "House"} | BitCraft Hub`
      : "House Details",
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
  <v-container>
    <div v-if="housePending" class="text-center py-10">
      <v-progress-circular indeterminate size="64" color="primary"></v-progress-circular>
    </div>

    <template v-else-if="house">
      <v-row>
        <v-col cols="12">
          <v-btn to="/houses" variant="text" prepend-icon="mdi-arrow-left" class="mb-4">
            Back to Houses
          </v-btn>
          <div class="d-flex align-center gap-4 mb-6">
            <h1 class="text-h3 font-weight-black">
              {{ house.owner_username || 'Unknown Owner' }}'s House
            </h1>
            <v-chip color="primary" label>{{ house.region }}</v-chip>
          </div>
        </v-col>
      </v-row>

      <v-row>
        <v-col cols="12">
          <v-card>
            <v-tabs v-model="tab" bg-color="surface">
              <v-tab value="overview">Overview</v-tab>
              <v-tab value="permissions">Permissions ({{ house.permissions.length }})</v-tab>
              <v-tab value="inventories">Inventories ({{ inventories?.inventories.length || 0 }})</v-tab>
              <v-tab value="changelog">Changes ({{ changelog?.length || 0 }})</v-tab>
            </v-tabs>

            <v-card-text>
              <v-window v-model="tab">
                <!-- Overview -->
                <v-window-item value="overview">
                  <v-row>
                    <v-col cols="12" md="6">
                      <v-list lines="two" :class="computedClass" class="rounded">
                        <v-list-item title="Entity ID" :subtitle="house.entity_id.toString()"></v-list-item>
                        <v-divider></v-divider>
                        <v-list-item title="Entrance Building ID" :subtitle="house.entrance_building_entity_id.toString()"></v-list-item>
                        <v-divider></v-divider>
                        <v-list-item title="Region" :subtitle="`${house.region} (Index: ${house.region_index})`"></v-list-item>
                      </v-list>
                    </v-col>
                    <v-col cols="12" md="6">
                      <v-list lines="two" :class="computedClass" class="rounded">
                        <v-list-item title="Rank" :subtitle="house.rank.toString()"></v-list-item>
                        <v-divider></v-divider>
                        <v-list-item title="Status" :subtitle="house.is_empty ? 'Empty' : 'Occupied'"></v-list-item>
                        <v-divider></v-divider>
                        <v-list-item title="Dimension ID" :subtitle="inventories?.dimension_id?.toString() || 'None'"></v-list-item>
                      </v-list>
                    </v-col>
                  </v-row>
                </v-window-item>

                <!-- Permissions -->
                <v-window-item value="permissions">
                  <v-table v-if="house.permissions.length > 0">
                    <thead>
                      <tr>
                        <th class="text-left">Allowed Entity ID</th>
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
                          <v-chip :color="p.rank === 7 ? 'gold' : 'blue-grey'" size="small">
                            {{ getRankName(p.rank) }}
                          </v-chip>
                        </td>
                      </tr>
                    </tbody>
                  </v-table>
                  <div v-else class="text-center py-6 text-grey">
                    No explicit permissions found.
                  </div>
                </v-window-item>

                <!-- Inventories -->
                <v-window-item value="inventories">
                  <div v-if="invPending" class="text-center py-6">
                    <v-progress-circular indeterminate></v-progress-circular>
                  </div>
                  <template v-else-if="inventories && inventories.inventories.length > 0">
                    <bitcraft-inventory
                      v-for="inv in inventories.inventories"
                      :key="inv.entity_id.toString()"
                      :inventory="inv"
                      class="mb-4"
                    />
                  </template>
                  <div v-else class="text-center py-6 text-grey">
                    No interior inventories found for this house.
                  </div>
                </v-window-item>

                <!-- Changelog -->
                <v-window-item value="changelog">
                  <div v-if="changelogPending" class="text-center py-6">
                    <v-progress-circular indeterminate></v-progress-circular>
                  </div>
                  <template v-else-if="changelog && changelog.length > 0">
                    <bitcraft-inventory-changes :items="changelog" />
                  </template>
                  <div v-else class="text-center py-6 text-grey">
                    No inventory changes found.
                  </div>
                </v-window-item>
              </v-window>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>
    </template>

    <div v-else class="text-center py-10">
      <v-icon icon="mdi-alert-circle-outline" size="64" color="error"></v-icon>
      <div class="text-h6 mt-4">House not found</div>
      <v-btn to="/houses" class="mt-4">Return to listing</v-btn>
    </div>
  </v-container>
</template>

<style scoped>
.gap-4 {
  gap: 16px;
}
</style>
