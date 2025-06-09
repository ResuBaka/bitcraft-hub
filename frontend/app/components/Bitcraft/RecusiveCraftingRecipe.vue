
<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const props = defineProps<{
  item: any;
  item_desc: any;
  cargo_desc: any;
}>();

function getDesc(item: any) {
  let desc;
  if (item.type == "Item") {
    desc = props.item_desc[item.id];
  } else {
    desc = props.cargo_desc[item.id];
  }
  return desc;
}
const desc = computed(() => {
  let desc;

  if (props.item.type == "Item") {
    desc = props.item_desc[props.item.id];
  } else {
    desc = props.cargo_desc[props.item.id];
  }
  return desc;
});
</script>
<template>
    <v-list-item>
      <v-badge :content="item.quantity" location="right" class="align-start">
        <v-img :src="iconAssetUrlNameRandom(desc.icon_asset_name).url" height="75" :width="item.type == 'Item' ? 75 : 128"></v-img>
      </v-badge>
      <v-list-item-title>Name: {{ desc.name }}</v-list-item-title>
      <template  v-if="item?.children?.length === 1">
        <recusive-crafting-recipe v-for="recipe_item in item.children[0].children" :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" ></recusive-crafting-recipe>
      </template>
      <template  v-else>
        <v-expansion-panels>
          <v-expansion-panel v-for="recipe in item.children"
            :title="getDesc(recipe.children[0]).name">
            <v-expansion-panel-text>
               <recusive-crafting-recipe v-for="recipe_item in recipe.children" :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" ></recusive-crafting-recipe>
            </v-expansion-panel-text>
          </v-expansion-panel>
      </v-expansion-panels>
    </template>
    </v-list-item>
</template>