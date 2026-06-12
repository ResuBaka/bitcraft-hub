<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { CommandPaletteGroup } from "@nuxt/ui";
import type { ItemCargo } from "~/types/ItemCargo";
import type { ItemsAndCargoResponse } from "~/types/ItemsAndCargoResponse";
import { rarityToTextClass, tierToTextClass } from "~/utils";

type ItemCommand = {
  label: string;
  value: string;
  item: ItemCargo;
  slot: "bitcraft-item";
};

const item = ref<string>("");
const open = ref(false);
const selectedItems = ref<ItemCommand[]>([]);
const router = useRouter();

const emit = defineEmits({
  search_changed(payload: string) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
  model_changed(payload: ItemCargo[]) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
});

const {
  data: itemsAndCargoData,
  pending,
  refresh,
} = await useLazyFetchMsPack<ItemsAndCargoResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};

      options.query.search = item.value;
      options.query.no_item_list = true;
      options.query.per_page = 50;

      if (Object.keys(options.query).length > 1) {
        const query = { item: item.value };
        router.push({ query });
      } else if (options.query.page < 1) {
        router.push({});
      }
    },
  },
);

watchThrottled(
  () => [item.value],
  () => {
    refresh();
  },
  { throttle: 50 },
);

const clearItems = () => {
  selectedItems.value = [];
  item.value = "";
};

const itemOptions = computed<ItemCommand[]>(() => {
  return (itemsAndCargoData.value?.items ?? []).map((item) => ({
    label: item.name,
    value: `${item.type}:${item.id}`,
    item,
    slot: "bitcraft-item",
  }));
});

const groups = computed<CommandPaletteGroup<ItemCommand>[]>(() => [
  {
    id: "items",
    label: item.value ? `Items matching "${item.value}"` : "Items",
    items: itemOptions.value.filter((item) => item.item.type === "Item"),
    ignoreFilter: true,
  },
  {
    id: "cargo",
    label: item.value ? `Cargo matching "${item.value}"` : "Cargo",
    items: itemOptions.value.filter((item) => item.item.type === "Cargo"),
    ignoreFilter: true,
  },
]);

const buttonLabel = computed(() => {
  if (selectedItems.value.length === 1) {
    return selectedItems.value[0].item.name;
  }
  if (selectedItems.value.length > 1) {
    return `${selectedItems.value.length} items selected`;
  }
  return "Search or select items";
});

watch(item, (value) => {
  emit("search_changed", value);
});

watch(selectedItems, (value) => {
  emit(
    "model_changed",
    value.map((selected) => selected.item),
  );
});
</script>

<template>
  <div class="inline-flex items-center gap-1">
    <UPopover v-model:open="open" :content="{ align: 'start' }">
      <UButton
        color="neutral"
        variant="subtle"
        icon="i-lucide-search"
        :label="buttonLabel"
        class="justify-start"
      />

      <template #content>
        <UCommandPalette
          v-model="selectedItems"
          v-model:search-term="item"
          :groups="groups"
          :loading="pending"
          multiple
          :fuse="{ resultLimit: 50 }"
          placeholder="Search items..."
          class="h-80 w-80 sm:w-96"
          :ui="{ input: '[&>input]:h-9 [&>input]:text-sm' }"
        >
          <template #bitcraft-item-leading="{ item: option }">
            <BitcraftInventoryImg :item="option.item" :width="36" :height="36" skip-error-text />
          </template>

          <template #bitcraft-item-label="{ item: option }">
            <div class="flex min-w-0 flex-col gap-0.5">
              <div class="truncate font-medium" :class="tierToTextClass(option.item.tier)">
                {{ option.item.name }}
              </div>
              <div class="flex items-center gap-2 text-xs">
                <span :class="tierToTextClass(option.item.tier)">T{{ option.item.tier }}</span>
                <span :class="rarityToTextClass(option.item.rarity)">{{ option.item.rarity }}</span>
                <span class="text-muted">{{ option.item.type }}</span>
              </div>
            </div>
          </template>
        </UCommandPalette>
      </template>
    </UPopover>
    <UButton
      v-if="selectedItems.length"
      color="neutral"
      variant="ghost"
      icon="i-lucide-x"
      aria-label="Clear selected items"
      @click="clearItems"
    />
  </div>
</template>
