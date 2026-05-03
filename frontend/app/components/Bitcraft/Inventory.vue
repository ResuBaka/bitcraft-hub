<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemCargo } from "~/types/ItemCargo";
import type { ResolvedInventory } from "~/types/ResolvedInventory";
import AutocompleteItem from "./autocomplete/AutocompleteItem.vue";
import AutocompleteUser from "./autocomplete/AutocompleteUser.vue";
import InventoryChanges from "./InventoryChanges.vue";
import { rarityToBorderClass } from "~/utils";

const props = defineProps<{
  inventory: ResolvedInventory;
}>();

const showChangelog = ref(false);
const playerId = ref<bigint | null>();
const itemObject = ref<ItemCargo | undefined>();
const numberFormat = new Intl.NumberFormat(undefined, {
  notation: "compact",
  compactDisplay: "short",
  maximumFractionDigits: 2,
  maximumSignificantDigits: 3,
});

const itemPockets = computed(() => {
  return props.inventory?.pockets.slice(0, props.inventory.cargo_index) ?? [];
});

const cargoPockets = computed(() => {
  return (
    props.inventory?.pockets.slice(
      props.inventory.pockets.length -
        (props.inventory.pockets.length - props.inventory.cargo_index),
    ) ?? []
  );
});

const inventoryLabel = computed(() => {
  if (!props.inventory) {
    return "Inventory";
  }
  return props.inventory.nickname || props.inventory.entity_id?.toString();
});

const { data: InventoryChangesFetch, refresh: InventoryChangesRefresh } = useFetchMsPack<
  InventoryChangelog[]
>(() => `/api/bitcraft/inventorys/changes/${props.inventory.entity_id}`, {
  onRequest: ({ options }) => {
    options.query = options.query || {};
    if (itemObject.value) {
      options.query.item_id = itemObject.value.id;
      options.query.item_type = itemObject.value.type;
    }
    if (playerId.value) {
      options.query.user_id = playerId.value.toString();
    }
    options.query.per_page = 20;
  },
});

watchThrottled(
  () => [itemObject.value, playerId.value],
  () => InventoryChangesRefresh(),
  { throttle: 50 },
);
</script>

<template>
  <div v-if="inventory" v-bind="$attrs" class="inventory-root">
    <UCard
      class="mb-6"
      :ui="{ header: 'p-4', body: 'p-0', footer: showChangelog ? 'p-4' : 'sm:p-0 p-4' }"
    >
      <template #header>
        <div class="inventory-header">
          <div class="inventory-title">
            <h3 class="inventory-name">{{ inventoryLabel }}</h3>
            <div class="inventory-stats-row">
              <div class="inventory-stat">
                <span class="inventory-stat-label">Item</span>
                <span class="inventory-stat-value">
                  {{ itemPockets.filter((pocket) => !!pocket.contents).length }}/{{
                    itemPockets.length
                  }}
                </span>
                <div class="inventory-stat-bar" aria-hidden="true"></div>
              </div>
              <div class="inventory-stat">
                <span class="inventory-stat-label">Cargo</span>
                <span class="inventory-stat-value">
                  {{ cargoPockets.filter((pocket) => !!pocket.contents).length }}/{{
                    cargoPockets.length
                  }}
                </span>
                <div class="inventory-stat-bar" aria-hidden="true"></div>
              </div>
            </div>
            <div v-if="inventory.claim" class="inventory-claim">
              <span>Claim</span>
              <NuxtLink
                class="inventory-claim-link"
                :to="{ name: 'claims-id', params: { id: inventory.claim.entity_id.toString() } }"
              >
                {{ inventory.claim.name }}
                <span class="inventory-claim-region">
                  (<bitcraft-region :region="inventory.claim.region" />)
                </span>
              </NuxtLink>
            </div>
          </div>
          <div class="inventory-controls">
            <USwitch v-model="showChangelog" label="Show changelog" />
          </div>
        </div>
      </template>

      <div>
        <div class="inventory-grid">
          <div
            v-for="(pocket, index) in inventory.pockets.filter((pocket) => !!pocket.contents)"
            :key="index"
            class="inventory-slot"
          >
            <UTooltip
              :ui="{
                content: 'h-full',
              }"
              :delay-duration="250"
            >
              <template #content>
                <div class="inventory-tooltip">
                  <div class="inventory-tooltip-title">
                    {{ pocket.contents.item.name }}
                  </div>
                  <div class="inventory-tooltip-sub">Rarity: {{ pocket.contents.item.rarity }}</div>
                  <div class="inventory-tooltip-sub">
                    Volume: {{ pocket.contents.quantity }}/{{
                      numberFormat.format(
                        pocket.volume /
                          (pocket.contents.item.volume === 0 ? 1 : pocket.contents.item.volume),
                      )
                    }}
                  </div>
                </div>
              </template>
              <div
                class="inventory-slot-box"
                :class="`background-color-tier-${pocket.contents.item.tier} ${rarityToBorderClass(pocket.contents.item.rarity)}`"
              >
                <div class="tier-label">T{{ pocket.contents.item.tier }}</div>
                <div class="item-icon">
                  <InventoryImg :item="pocket.contents.item" width="70%" height="70%" />
                </div>
                <div
                  class="quantity-badge"
                  :class="
                    pocket.volume /
                      (pocket.contents.item.volume === 0 ? 1 : pocket.contents.item.volume) ==
                    pocket.contents.quantity
                      ? 'text-green-700'
                      : ''
                  "
                >
                  {{ numberFormat.format(pocket.contents.quantity) }}
                </div>
              </div>
            </UTooltip>
          </div>
        </div>
      </div>

      <template #footer>
        <div v-if="showChangelog" class="inventory-changelog">
          <div class="inventory-changelog-header">History &amp; Changes</div>
          <div class="inventory-filters">
            <AutocompleteUser @model_changed="(item) => (playerId = item)" />
            <AutocompleteItem @model_changed="(item) => (itemObject = item)" />
          </div>
          <InventoryChanges :items="InventoryChangesFetch" />
        </div>
      </template>
    </UCard>
  </div>
</template>

<style scoped>
.inventory-root {
  width: 100%;
}

.inventory-header {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  justify-content: space-between;
  align-items: flex-start;
}

.inventory-title {
  display: grid;
  gap: 6px;
}

.inventory-name {
  font-size: 1.4rem;
  font-weight: 700;
  margin: 0;
}

.inventory-claim {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 0.9rem;
  color: rgba(148, 163, 184, 0.9);
}

.inventory-claim-link {
  font-weight: 600;
  color: rgb(var(--color-primary-500));
  text-decoration: none;
}

.inventory-claim-link:hover {
  text-decoration: underline;
}

.inventory-claim-region {
  font-weight: 500;
  color: rgba(148, 163, 184, 0.8);
}

.inventory-controls {
  display: flex;
  align-items: center;
  gap: 12px;
}

.inventory-stats-row {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  align-items: center;
}

.inventory-stat {
  display: grid;
  grid-template-columns: auto auto;
  column-gap: 6px;
  row-gap: 4px;
  align-items: baseline;
  min-width: 140px;
}

.inventory-stat-label {
  display: inline-block;
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.12em;
  color: rgba(148, 163, 184, 0.8);
}

.inventory-stat-value {
  font-size: 0.9rem;
  font-weight: 600;
  color: rgb(var(--color-gray-200));
}

.inventory-stat-bar {
  grid-column: 1 / -1;
  width: 120px;
  height: 2px;
  border-radius: 999px;
  background: rgba(148, 163, 184, 0.35);
}

.inventory-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, 80px);
  gap: 10px;
  justify-content: start;
}

.inventory-slot {
  display: flex;
  justify-content: center;
  align-items: center;
}

.inventory-slot-box {
  width: 80px;
  height: 80px;
  cursor: default;
  overflow: hidden;
  transition: all 0.2s ease;
  border: 4px solid rgba(148, 163, 184, 0.2);
  display: grid;
  place-items: center;
  position: relative;
  box-shadow: 0 10px 20px rgba(15, 23, 42, 0.2);
}

.item-icon {
  opacity: 0.8;
  user-select: none;
  width: 100%;
  height: 100%;
  display: grid;
  place-items: center;
  padding: 6px;
}

.quantity-badge {
  position: absolute;
  bottom: 0px;
  right: 4px;
  font-size: 0.9rem;
  font-weight: 800;
  text-shadow: 0px 0px 3px rgba(15, 23, 42, 0.6);
}

.tier-label {
  position: absolute;
  top: 8px; /* Adjusted to sit just below or on the 4px border */
  left: 4px;
  font-size: 0.9rem;
  font-weight: 900;
  line-height: 1;
  text-transform: uppercase;
  user-select: none;
  /* Optional: gives it a slight shadow to pop against dark icons */
  color: rgba(226, 232, 240, 0.95);
  text-shadow: 0px 0px 2px rgba(15, 23, 42, 0.6);
}

.inventory-tooltip {
  text-align: center;
  padding: 6px 8px;
}

.inventory-tooltip-title {
  font-size: 0.9rem;
  font-weight: 600;
  text-transform: uppercase;
}

.inventory-tooltip-sub {
  font-size: 0.75rem;
  color: rgba(148, 163, 184, 0.85);
}

.inventory-changelog {
  display: grid;
  gap: 16px;
}

.inventory-changelog-header {
  font-size: 1rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

.inventory-filters {
  display: grid;
  gap: 12px;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
}

@media (max-width: 640px) {
  .inventory-controls {
    width: 100%;
  }

  .inventory-grid {
    grid-template-columns: repeat(auto-fill, 72px);
  }

  .inventory-slot-box {
    width: 72px;
    height: 72px;
  }
}
</style>
