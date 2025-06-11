
<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const props = defineProps<{
  id: any;
  type: any;
  quantity: any;
  item_desc: any;
  cargo_desc: any;
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
    <v-list-item >
      <v-badge :content="Intl.NumberFormat().format(quantity)" location="right" class="align-start">
        <v-img :src="iconAssetUrlNameRandom(desc.icon_asset_name).url" height="75" :width="type == 'Item' ? 75 : 128"></v-img>
      </v-badge>
      <v-list-item-title>{{ desc.name }}</v-list-item-title>
    </v-list-item>
</template>