<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import AutocompleteUser from "./autocomplete/AutocompleteUser.vue";
import AutocompleteItem from "./autocomplete/AutocompleteItem.vue";
import InventoryChanges from "./InventoryChanges.vue";
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemCargo } from "~/types/ItemCargo";

const { inventory } = defineProps<{
  inventory: any;
}>();

const router = useRouter();

const playerId = ref<bigint | null>();

const itemObject = ref<ItemCargo | undefined>();

const { data: InventoryChangesFetch, refresh: InventoryChangesRefresh } =
  useFetchMsPack<InventoryChangelog[]>(
    () => {
      return `/api/bitcraft/inventorys/changes/${inventory.entity_id}`;
    },
    {
      onRequest: ({ options }) => {
        options.query = options.query || {};

        if (itemObject.value !== undefined && itemObject.value !== null) {
          options.query.item_id = itemObject.value.id;
          options.query.item_type = itemObject.value.type;
        }
        if (playerId.value !== undefined && playerId.value !== null) {
          options.query.user_id = playerId.value.toString();
        }
        options.query.per_page = 20;

        if (Object.keys(options.query).length > 1) {
          const query = { ...options.query };
          delete query.per_page;
          router.push({ query });
        } else if (options.query.page < 1) {
          router.push({});
        }
      },
    },
  );

const inventoryChanges = computed(() => {
  return InventoryChangesFetch.value ?? [];
});

const headersPockets = [
  { title: "Name", key: "contents.item.name" },
  { title: "Quantity", key: "contents.quantity", align: "end" },
];

const backgroundColorRow = ({ index }: { index: number }) => {
  return {
    class: index % 2 === 0 ? "" : "bg-surface-light",
  };
};

watchThrottled(
  () => [itemObject.value, playerId.value],
  (value, oldValue) => {
    InventoryChangesRefresh();
  },
  { throttle: 50 },
);
</script>

<template>
  <template v-if="inventory !== undefined">
    <div v-bind="$attrs">
      <v-card class="mb-5">
        <v-toolbar color="transparent">
          <v-toolbar-title>Inventory: <strong>{{ inventory.nickname ? inventory.nickname : inventory.entity_id }}</strong></v-toolbar-title>

        </v-toolbar>

        <v-card-text>
          <v-card-title>Current Items</v-card-title>
          <v-data-table density="compact" :headers="headersPockets"
                        :items="inventory.pockets" :row-props="backgroundColorRow">
          </v-data-table>
        </v-card-text>
      </v-card>
      <v-spacer></v-spacer>
      <v-card>
        <v-card-title>Changes</v-card-title>
        <v-card-text>
          <v-row>
            <v-col>
                <autocomplete-user @model_changed="(item) => playerId=item" />
          </v-col>
            <v-col>
              <autocomplete-item
                    @model_changed="(item) => itemObject=item"
                />
            </v-col>
          </v-row>
          <inventory-changes :items="InventoryChangesFetch"/>
        </v-card-text>
      </v-card>
    </div>
  </template>
</template>

<style scoped>
</style>