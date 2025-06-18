<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { ItemCargo } from "~/types/ItemCargo";
import type { ItemsAndCargoResponse } from "~/types/ItemsAndCargoResponse";

const item = ref<string | undefined>();
const item_object = ref<ItemCargo | undefined>();
const router = useRouter();

const emit = defineEmits({
  search_changed(payload: string) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
  model_changed(payload: ItemCargo) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
});

const {
  public: { api },
} = useRuntimeConfig();

const { data: itemsAndCargoData, refresh } =
  await useLazyFetchMsPack<ItemsAndCargoResponse>(
    () => {
      return `${api.base}/api/bitcraft/itemsAndCargo`;
    },
    {
      onRequest: ({ options }) => {
        options.query = options.query || {};

        options.query.search = item.value;
        options.query.no_item_list = true;
        options.query.per_page = 20;

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
  (value, oldValue) => {
    refresh();
  },
  { throttle: 50 },
);
</script>

<template>
    <v-autocomplete
                @update:model-value="(item) => $emit('model_changed', item)"
                @update:search="(item) => $emit('search_changed', item)"
                v-model="item_object"
                v-model:search="item"
                :items="itemsAndCargoData?.items || []"
                :item-title="item=>`${item.name} - ${item.rarity}`"
                item-value="name"
                :return-object="true"
                label="item"
                outlined
                dense
                clearable
            ></v-autocomplete>
</template>