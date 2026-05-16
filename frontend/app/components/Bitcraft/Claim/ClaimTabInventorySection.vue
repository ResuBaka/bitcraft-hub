<script setup lang="ts">
import { computed } from "vue";
import type { ExpendedRefrence } from "~/types/ExpendedRefrence";
import { rarityToTextClass, tierToBorderClassByLevel } from "~/utils";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";

const props = defineProps<{
  search: string | null;
  tier: number | null;
  rarity: string | null;
  tierOptions: number[];
  rarityOptions: string[];
  items: ExpendedRefrence[];
  page: number;
  total: number;
  itemsPerPage: number;
  searchPlaceholder: string;
  locationSection: string;
  toolMode?: boolean;
  showTotalItems?: boolean;
  tierToColor: Record<number, string>;
  numberFormat: Intl.NumberFormat;
  openItemLocationModal: (
    inventory: ExpendedRefrence,
    section: string,
    source?: "inventory" | "tool",
  ) => void;
}>();

const emit = defineEmits<{
  "update:search": [value: string | null];
  "update:tier": [value: number | null];
  "update:rarity": [value: string | null];
  "update:page": [value: number];
}>();

const searchModel = computed({ get: () => props.search, set: (v) => emit("update:search", v) });
const tierModel = computed({ get: () => props.tier, set: (v) => emit("update:tier", v) });
const rarityModel = computed({ get: () => props.rarity, set: (v) => emit("update:rarity", v) });
const pageModel = computed({ get: () => props.page, set: (v) => emit("update:page", v) });

const totalItems = computed(() =>
  props.items.reduce((accumulator, currentValue) => accumulator + currentValue.quantity, 0),
);

const openLocation = (inventory: ExpendedRefrence) => {
  props.openItemLocationModal(
    inventory,
    props.locationSection,
    props.toolMode ? "tool" : "inventory",
  );
};
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap gap-2">
      <UInput
        v-model="searchModel"
        icon="i-heroicons-magnifying-glass"
        :placeholder="searchPlaceholder"
        class="w-full sm:w-64"
      />
      <USelect v-model="tierModel" :items="tierOptions" placeholder="Tier" class="w-32" />
      <USelectMenu
        v-model="rarityModel"
        :items="rarityOptions"
        placeholder="Rarity"
        clear
        class="w-40"
      />
      <span v-if="showTotalItems" class="align-middle"> Total Items: {{ totalItems }} </span>
    </div>

    <div
      class="grid gap-2 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6"
    >
      <UCard
        v-for="inventory in items"
        :key="`${inventory.item_id}-${inventory.item_type}`"
        class="inventory-card border-l-4"
        :class="tierToBorderClassByLevel(inventory.item.tier)"
        role="button"
        tabindex="0"
        :ui="{ header: 'p-3', body: 'hidden' }"
        @click="openLocation(inventory)"
        @keydown.enter="openLocation(inventory)"
        @keydown.space.prevent="openLocation(inventory)"
      >
        <template #header>
          <div class="inventory-card__header inventory-card__header--media">
            <div class="inventory-card__meta">
              <InventoryImg :item="inventory.item" :width="48" :height="48" />
              <div class="inventory-card__text">
                <div class="inventory-card__title" :class="tierToColor[inventory.item.tier]">
                  {{ inventory.item.name }}
                </div>
                <div
                  class="inventory-card__subtitle"
                  :class="rarityToTextClass(inventory.item.rarity)"
                >
                  {{ inventory.item.rarity }}
                </div>
              </div>
            </div>
            <div class="inventory-card__qty">{{ numberFormat.format(inventory.quantity) }}</div>
          </div>
        </template>
      </UCard>
    </div>

    <div class="flex justify-center">
      <UPagination v-model:page="pageModel" :total="total" :items-per-page="itemsPerPage" />
    </div>
  </div>
</template>

<style scoped>
.inventory-card {
  display: grid;
  text-align: left;
  transition:
    transform 0.2s ease,
    box-shadow 0.2s ease,
    border-color 0.2s ease;
  min-height: 0;
  cursor: pointer;
}
.inventory-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 12px 24px -18px rgba(15, 23, 42, 0.4);
}
.inventory-card__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}
.inventory-card__header--media {
  min-height: 48px;
}
.inventory-card__meta {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}
.inventory-card__text {
  min-width: 0;
}
.inventory-card__title {
  text-transform: uppercase;
  font-size: 1rem;
  line-height: 1.1;
}
.inventory-card__subtitle {
  font-size: 0.75rem;
}
.inventory-card__qty {
  font-size: 1rem;
  white-space: nowrap;
}
</style>
