<script setup lang="ts">
import type { BuildingDescRow } from "~/modules/bitcraft/gamestate/buildingDesc";

const {
  public: { iconDomain },
} = useRuntimeConfig();
const imagedErrored = ref(false);

const { building } = defineProps<{
  building: BuildingDescRow;
}>();
const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});
</script>

<template>
  <v-card>
    <v-card-item>
      <template #prepend v-if="iconDomain && imagedErrored !== true && building.icon_asset_name">
        <v-img @error="imagedErrored = true" :src="`${iconDomain}/${building.icon_asset_name}.png`" height="50" width="50"></v-img>
      </template>
      <v-card-title>
        <nuxt-link
            class="text-decoration-none text-high-emphasis font-weight-black"
            :to="{ name: 'buildings-id', params: { id: building.id } }"
        >
          {{ building.name }} ({{ building.count }})
        </nuxt-link>
      </v-card-title>
    </v-card-item>
    <v-card-text :class="computedClass" class="pb-1">
      <v-table density="compact" :class="computedClass">
        <tbody>
          <tr>
            <th>Tier:</th>
            <td>{{ building.functions[0]?.level }}</td>
          </tr>
          <tr>
            <th>Description:</th>
            <td>{{ building.description }}</td>
          </tr>
        </tbody>
      </v-table>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>