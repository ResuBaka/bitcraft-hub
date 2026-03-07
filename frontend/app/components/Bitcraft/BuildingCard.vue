<script setup lang="ts">
import type { BuildingDescRow } from "~/modules/bitcraft/gamestate/buildingDesc";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const imagedErrored = ref(false);

const { building } = defineProps<{
  building: BuildingDescRow;
}>();
const colorMode = useColorMode();

const computedClass = computed(() => {
  const isDark = colorMode.value == "dark";

  return {
    "bg-surface-light": isDark,
    "bg-grey-lighten-3": !isDark,
  };
});

const iconUrl = computed(() => {
  if (!building.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(building.icon_asset_name);
});
</script>

<template>
<!--  <v-card>-->
<!--    <v-card-item>-->
<!--      <template #prepend v-if="iconUrl.show && imagedErrored !== true">-->
<!--        <v-img @error="imagedErrored = true" :src="iconUrl.url" height="50" width="50"></v-img>-->
<!--      </template>-->
<!--      <v-card-title>-->
<!--        <nuxt-link-->
<!--            class="text-decoration-none text-high-emphasis font-weight-black"-->
<!--            :to="{ name: 'buildings-id', params: { id: building.id } }"-->
<!--        >-->
<!--          {{ building.name }} ({{ building.count }})-->
<!--        </nuxt-link>-->
<!--      </v-card-title>-->
<!--    </v-card-item>-->
<!--    <v-card-text :class="computedClass" class="pb-1">-->
<!--      <v-table density="compact" :class="computedClass">-->
<!--        <tbody>-->
<!--        <tr>-->
<!--          <th>Tier:</th>-->
<!--          <td>{{ building.functions[0]?.level }}</td>-->
<!--        </tr>-->
<!--        <tr>-->
<!--          <th>Description:</th>-->
<!--          <td>{{ building.description }}</td>-->
<!--        </tr>-->
<!--        </tbody>-->
<!--      </v-table>-->
<!--    </v-card-text>-->
<!--  </v-card>-->
</template>

<style scoped>

</style>