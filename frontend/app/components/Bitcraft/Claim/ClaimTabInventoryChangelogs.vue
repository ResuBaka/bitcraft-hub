<script setup lang="ts">
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemCargo } from "~/types/ItemCargo";
import AutocompleteUser from "~/components/Bitcraft/autocomplete/AutocompleteUser.vue";
import AutocompleteItem from "~/components/Bitcraft/autocomplete/AutocompleteItem.vue";
import InventoryChanges from "~/components/Bitcraft/InventoryChanges.vue";

defineProps<{
  items?: InventoryChangelog[];
}>();

const emit = defineEmits<{
  playerChanged: [value: bigint | null | undefined];
  itemChanged: [value: ItemCargo | undefined];
}>();
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap gap-2">
      <AutocompleteUser @model_changed="(item) => emit('playerChanged', item)" />
      <AutocompleteItem @model_changed="(item) => emit('itemChanged', item)" />
    </div>
    <InventoryChanges :items="items" />
  </div>
</template>
