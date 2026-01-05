<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import AutocompleteUser from "./autocomplete/AutocompleteUser.vue";
import AutocompleteItem from "./autocomplete/AutocompleteItem.vue";
import InventoryChanges from "./InventoryChanges.vue";
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemCargo } from "~/types/ItemCargo";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";

const props = defineProps<{
  inventory: any;
}>();

const showChangelog = ref(false);
const playerId = ref<bigint | null>();
const itemObject = ref<ItemCargo | undefined>();

const { data: InventoryChangesFetch, refresh: InventoryChangesRefresh } =
  useFetchMsPack<InventoryChangelog[]>(
    () => `/api/bitcraft/inventorys/changes/${props.inventory.entity_id}`,
    {
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
    },
  );

watchThrottled(
  () => [itemObject.value, playerId.value],
  () => InventoryChangesRefresh(),
  { throttle: 50 },
);
</script>

<template>
  <template v-if="inventory">
    <div v-bind="$attrs">
      <v-card class="mb-5" elevation="2">
        <v-list-item class="pa-4">
          <template #title>
            <span class="text-h6">Inventory: </span>
            <strong class="text-secondary">{{ inventory.nickname || inventory.entity_id }} </strong>
            <template v-if="inventory.claim">
              &nbsp;
              <span class="text-h6">Claim: </span>
              <nuxt-link class="text-primary text-decoration-none text-high-emphasis font-weight-black"
                         :to="{ name: 'claims-id', params: { id: inventory.claim.entity_id } }"
              >
                <strong>{{ inventory.claim.name }} (<bitcraft-region :region="inventory.claim.region" />)</strong>
              </nuxt-link>

            </template>
          </template>
          <template #append>
            <v-checkbox v-model="showChangelog" label="Show Changelog" hide-details density="compact"></v-checkbox>
          </template>
        </v-list-item>

        <v-divider></v-divider>

        <v-card-text>
          <div class="mb-4 d-flex justify-space-between align-center">
            <div class="text-subtitle-2 text-medium-emphasis">
              Occupied Slots: {{ inventory.pockets.filter(p => !!p.contents).length }} / {{ inventory.pockets.length }}
            </div>
          </div>

          <v-row dense class="inventory-container pa-2 rounded-lg">
            <v-col
                v-for="(pocket, index) in inventory.pockets.filter(pocket => !!pocket.contents)"
                :key="index"
                cols="4"
                sm="4"
                md="3"
                xl="1"
                lg="2"
                class="d-flex justify-center"
            >
              <v-sheet
                  border
                  rounded
                  class="inventory-slot-box d-flex align-center justify-center position-relative"
                  :class="{ 'has-content': !!pocket.contents }"
                  :elevation="pocket.contents ? 2 : 0"
              >
                <template v-if="pocket.contents">
                  <v-tooltip activator="parent" location="top" transition="fade-transition">
                    <div class="text-center">
                      <div :class="`font-weight-bold text-${getTierColor(pocket.contents.item.tier)} text-uppercase`">
                        {{ pocket.contents.item.name }}
                      </div>
                      <div class="text-caption">Rarity: {{ pocket.contents.item.rarity }}</div>
                    </div>
                  </v-tooltip>

                  <div class="tier-border" :class="`bg-${getTierColor(pocket.contents.item.tier)}`"></div>

                  <div class="item-icon text-h6 font-weight-black">
                    <inventory-img :item="pocket.contents.item" />
                  </div>

                  <div class="quantity-badge">
                    {{ pocket.contents.quantity }}/{{ pocket.volume / (pocket.contents.item.volume == 0 ? 1 : pocket.contents.item.volume) }}
                  </div>
                </template>

                <template v-else>
                  <v-icon icon="mdi-dots-grid" color="disabled" size="small" opacity="0.2"></v-icon>
                </template>
              </v-sheet>
            </v-col>
          </v-row>
        </v-card-text>
      </v-card>

      <v-fade-transition>
        <v-card v-if="showChangelog" class="mt-4 shadow-lg">
          <v-card-title class="bg-grey-lighten-4">History & Changes</v-card-title>
          <v-card-text class="pt-4">
            <v-row dense>
              <v-col cols="12" sm="6">
                <autocomplete-user @model_changed="(item) => playerId = item" />
              </v-col>
              <v-col cols="12" sm="6">
                <autocomplete-item @model_changed="(item) => itemObject = item" />
              </v-col>
            </v-row>
            <v-divider class="my-4"></v-divider>
            <inventory-changes :items="InventoryChangesFetch" />
          </v-card-text>
        </v-card>
      </v-fade-transition>
    </div>
  </template>
</template>

<style scoped>
.inventory-container {
  background-color: rgba(var(--v-border-color), 0.05);
  min-height: 100px;
}

.inventory-slot-box {
  width: 100%;
  aspect-ratio: 1 / 1;
  background-color: rgb(var(--v-theme-surface));
  cursor: default;
  overflow: hidden;
  transition: all 0.2s ease;
}

.inventory-slot-box.has-content {
  cursor: pointer;
}

.inventory-slot-box.has-content:hover {
  transform: translateY(-2px);
  border-color: rgba(var(--v-theme-primary), 0.5) !important;
}

/* Color strip at the top of the slot indicating item tier */
.tier-border {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 4px;
}

.item-icon {
  opacity: 0.8;
  user-select: none;
}

.quantity-badge {
  position: absolute;
  bottom: 0px;
  right: 4px;
  font-size: 0.7rem;
  font-weight: 800;
  color: rgb(var(--v-theme-on-surface));
  text-shadow: 0px 0px 3px rgb(var(--v-theme-surface));
}
</style>