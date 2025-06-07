<script setup lang="ts">
import type { ItemRow } from "~/modules/bitcraft/gamestate/item";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
const imagedErrored = ref(false);

const dev = import.meta.dev;

const { item } = defineProps<{
  item: ItemRow;
}>();

const {
  public: { iconDomain, api },
} = useRuntimeConfig();

const {
  data: neededInCrafting,
  execute: neededInCraftingExecute,
  status: neededInCraftingStatus,
} = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/recipes/needed_in_crafting/${item.id}`;
  },
  {
    immediate: false,
  },
);

const {
  data: producedInCrafting,
  execute: producedInCraftingExecute,
  status: producedInCraftingStatus,
} = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/recipes/produced_in_crafting/${item.id}`;
  },
  {
    immediate: false,
  },
);

const {
  data: neededToCraft,
  execute: neededToCraftExecute,
  status: neededToCraftStatus,
} = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/recipes/needed_to_craft/${item.id}`;
  },
  {
    immediate: false,
  },
);

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

const toggleContentToShow = (contetentArg: string) => {
  if (contetentArg === "neededToCraft") {
    if (
      neededToCraftData.value.length === 0 &&
      neededToCraftStatus.value !== "sucess"
    ) {
      neededToCraftExecute();
    }
  }

  if (contetentArg === "neededInCrafting") {
    if (
      neededInCraftingData.value.length === 0 &&
      neededInCraftingStatus.value !== "sucess"
    ) {
      neededInCraftingExecute();
    }
  }

  if (contetentArg === "producedInCrafting") {
    if (
      producedInCraftingData.value.length === 0 &&
      producedInCraftingStatus.value !== "sucess"
    ) {
      producedInCraftingExecute();
    }
  }

  contetentToShow.value = contetentArg;
};

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
      <v-card-title :class="`color-tier-${item.tier}`">
        <nuxt-link
          :class="`text-decoration-none color-tier-${item.tier}`"
          :to="{ name: 'items-type-id', params: { id: item.id, type: item.type } }"
        >
        {{ item.name }}
        </nuxt-link>
      </v-card-title>
      <v-card-subtitle :class="`color-tier-${item.tier}`">
        <template v-if="dev">
          Id: {{ item.id }}
        </template>
         Tier: {{ item.tier }}
         Tag: {{ item.tag }}
      </v-card-subtitle>
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
                @click="toggleContentToShow('neededToCraft')"
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
                @click="toggleContentToShow('neededInCrafting')"
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
                @click="toggleContentToShow('producedInCrafting')"
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
        <v-table :class="computedClass" density="compact" style="padding-top: 10px">
          <tbody>
          <tr >
            <th style='text-align: left'>Description:</th>
            <td style='text-align: right'>{{ item.description }}</td>
          </tr>
          <tr>
            <th style='text-align: left'>Volume:</th>
            <td style='text-align: right'>{{ item.volume }}</td>
          </tr>
          <tr>
            <th style='text-align: left'>Effort:</th>
            <td style='text-align: right'>{{ producedInCraftingData.length ? producedInCraftingData[0].actions_required : 0 }}</td>
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