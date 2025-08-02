
<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";

const props = defineProps<{
  id: number;
  type: "Cargo" | "Item";
  quantity: number;
  item_desc: { [key in number]?: ItemDesc };
  cargo_desc: { [key in number]?: CargoDesc };
}>();
const desc = computed(() => {
  let desc;

  if (props.type == "Item") {
    desc = props.item_desc[props.id];
  } else {
    desc = props.cargo_desc[props.id];
  }
  return desc;
});
</script>
<template>
    <v-list-item v-if="desc !== undefined" >
      <v-badge :content="Intl.NumberFormat().format(quantity)" location="right" class="align-start" offset-x="-10">
        <v-list-item-title class="align-content-center">{{ desc.name }}</v-list-item-title>
        <v-img :src="iconAssetUrlNameRandom(desc.icon_asset_name).url" height="75" :width="type == 'Item' ? 75 : 128"></v-img>
      </v-badge>
    </v-list-item>
</template>