<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type {
  objectWithChildren,
  PannelIndexs,
} from "~/pages/items/[type]/[id].vue";
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";

const props = defineProps<{
  item: objectWithChildren;
  item_desc: { [key in number]?: ItemDesc };
  cargo_desc: { [key in number]?: CargoDesc };
  pannel_indexs: PannelIndexs;
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
    <v-list-item v-if="item.deleted === undefined && desc !== undefined">
      <v-badge :content="Intl.NumberFormat().format(item.quantity)" location="right" class="align-start">
        <v-img :src="iconAssetUrlNameRandom(desc.icon_asset_name).url" height="75" :width="item.type == 'Item' ? 75 : 128"></v-img>
      </v-badge>
      <v-list-item-title>Name: {{ desc.name }}</v-list-item-title>
      <template  v-if="item?.children?.length === 1">
        <recusive-crafting-recipe v-if="item.children[0] !== undefined" v-for="(recipe_item, index) in item.children[0].children" :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" :pannel_indexs="pannel_indexs.children[0].children[index]" ></recusive-crafting-recipe>
      </template>
      <template  v-else>
        <v-expansion-panels v-model="pannel_indexs.pannels">
          <v-expansion-panel v-for="(recipe, index) in item.children"
            :title="getDesc(recipe.children[0]).name"
            :value="index">
            <v-expansion-panel-text>
               <recusive-crafting-recipe v-for="(recipe_item,index2) in recipe.children" :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" :pannel_indexs="pannel_indexs.children[index].children[index2]"  ></recusive-crafting-recipe>
            </v-expansion-panel-text>
          </v-expansion-panel>
      </v-expansion-panels>
    </template>
    </v-list-item>
</template>