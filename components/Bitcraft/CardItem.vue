<script setup lang="ts">
import type { ItemRow } from "~/modules/bitcraft/gamestate/item";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
const imagedErrored = ref(false);

const { item } = defineProps<{
  item: ItemRow;
}>();
const {
  public: { iconDomain },
} = useRuntimeConfig();

const { data: neededInCrafting } = await useLazyFetch("/api/bitcraft/recipes", {
  query: {
    neededInCrafting: item.id,
  },
});

const { data: producedInCrafting } = await useLazyFetch(
  "/api/bitcraft/recipes",
  {
    query: {
      producedInCrafting: item.id,
    },
  },
);

const { data: neededToCraft } = await useLazyFetch("/api/bitcraft/recipes", {
  query: {
    neededToCraft: item.id,
  },
});

const neededInCraftingData = computed(() => {
  return neededInCrafting.value ?? [];
});

const producedInCraftingData = computed(() => {
  return producedInCrafting.value ?? [];
});

const neededToCraftData = computed(() => {
  return neededToCraft.value ?? [];
});

const contetentToShow = ref("default");

const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

const iconUrl = computed(() => {
  if (!item.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(item.icon_asset_name);
});
</script>

<template>
  <v-card density="compact">
    <v-card-item>
      <template #prepend v-if="iconUrl.show && imagedErrored !== true">
        <v-img @error="imagedErrored = true" :src="iconUrl.url" height="50" width="50"></v-img>
      </template>
      <v-card-title>{{ item.name }}</v-card-title>
      <template v-slot:append>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'default'"
            >
              <v-icon>
                mdi-home
              </v-icon>
            </v-btn>
          </template>
          <span>Programmatic tooltip</span>
        </v-tooltip>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'neededToCraft'"
            >
              <v-icon>
                mdi-chart-bar-stacked
              </v-icon>
            </v-btn>
          </template>
          <span>neededToCraft</span>
        </v-tooltip>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'neededInCrafting'"
            >
              <v-badge color="primary" :content="neededInCraftingData.length">
                <v-icon>
                  mdi-call-split
                </v-icon>
              </v-badge>
            </v-btn>
          </template>
          <span>neededInCrafting</span>
        </v-tooltip>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'producedInCrafting'"
            >
              <v-badge color="primary" :content="producedInCraftingData.length">
                <v-icon>
                  mdi-call-split
                </v-icon>
              </v-badge>
            </v-btn>
          </template>
          <span>producedInCrafting</span>
        </v-tooltip>
      </template>
    </v-card-item>
    <v-card-text :class="computedClass">
      <template v-if="contetentToShow == 'default'">
        <v-table :class="computedClass" density="compact">
          <tbody>
          <tr style='text-align: right'>
            <th>Description:</th>
            <td>{{ item.description }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>Volume:</th>
            <td>{{ item.volume }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>Tag:</th>
            <td>{{ item.tag }}</td>
          </tr>
          <tr style='text-align: right'>
            <th>Tier:</th>
            <td>{{ item.tier }}</td>
          </tr>
          </tbody>
        </v-table>
      </template>
      <template v-if="contetentToShow == 'neededInCrafting'">
        <v-list :class="computedClass">
          <v-list-item v-for="crafting in neededInCraftingData" :key="crafting.id">
            <v-list-item-title>
              <bitcraft-card-item-crafting-name :item="item" :template="crafting.name"
                                                :craftId="crafting.crafted_item_stacks[0].item_id"></bitcraft-card-item-crafting-name>
            </v-list-item-title>
            <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
          </v-list-item>
        </v-list>
      </template>
      <template v-if="contetentToShow == 'neededToCraft'">
        <v-list>
          <bitcraft-item-stack :items="neededToCraft"></bitcraft-item-stack>
        </v-list>
      </template>
      <template v-if="contetentToShow == 'producedInCrafting'">
        <v-list :class="computedClass">
          <v-list-item v-for="crafting in producedInCraftingData" :key="crafting.id">
            <v-list-item-title>
              <bitcraft-card-item-crafting-name :item="item" :template="crafting.name"
                                                :craftId="crafting.crafted_item_stacks[0].item_id"></bitcraft-card-item-crafting-name>
            </v-list-item-title>
            <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
          </v-list-item>
        </v-list>
      </template>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>