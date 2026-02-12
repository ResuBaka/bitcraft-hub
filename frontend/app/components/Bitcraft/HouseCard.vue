<script setup lang="ts">
import type { HouseResponse } from "~/types/HouseResponse";
import { useTierColor } from "~/composables/useTierColor";

const props = defineProps<{
  house: HouseResponse;
}>();

const tierColors = useTierColor();
const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

const rankColor = computed(() => {
  return tierColors[props.house.rank as keyof typeof tierColors] || "grey";
});
</script>

<template>
  <v-card :border="true" class="mb-4">
    <v-card-item>
      <template #prepend>
        <v-avatar :color="rankColor" size="48">
          <v-icon icon="mdi-home-variant-outline" color="white"></v-icon>
        </v-avatar>
      </template>
      <v-card-title>
        <nuxt-link
          class="text-decoration-none text-high-emphasis font-weight-black"
          :to="{ name: 'players-id', params: { id: house.owner_entity_id.toString() } }"
        >
          {{ house.owner_username || 'Unknown Owner' }}'s House
        </nuxt-link>
      </v-card-title>
      <v-card-subtitle>
        Rank {{ house.rank }} | Region {{ house.region }}
      </v-card-subtitle>
    </v-card-item>

    <v-card-text :class="computedClass" class="pb-2">
      <div class="d-flex flex-column gap-1">
        <div class="d-flex justify-space-between">
          <span class="font-weight-bold">Entity ID:</span>
          <span>{{ house.entity_id.toString() }}</span>
        </div>
        <div class="d-flex justify-space-between">
          <span class="font-weight-bold">Entrance Building:</span>
          <span>{{ house.entrance_building_entity_id.toString() }}</span>
        </div>
        <div class="d-flex justify-space-between">
          <span class="font-weight-bold">Status:</span>
          <v-chip
            :color="house.is_empty ? 'grey' : 'success'"
            size="x-small"
            label
          >
            {{ house.is_empty ? 'Empty' : 'Occupied' }}
          </v-chip>
        </div>
      </div>
    </v-card-text>

    <v-divider></v-divider>

    <v-card-actions>
      <v-spacer></v-spacer>
      <v-btn
        color="primary"
        variant="text"
        :to="{ name: 'players-id', params: { id: house.owner_entity_id.toString() } }"
      >
        View Details
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<style scoped>
.gap-1 {
  gap: 4px;
}
</style>
