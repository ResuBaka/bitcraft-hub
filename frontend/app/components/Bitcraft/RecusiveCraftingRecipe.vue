
<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const props = defineProps<{
  recipeId: number;
  recipeInfo: any;
  recipies: Array<number>;
}>();
const exists =
  props.recipies.findIndex((value) => value == props.recipeId) === -1;
props.recipies.push(props.recipeId);
</script>
<template >
<template  v-if="exists" v-for = "item in recipeInfo.allRecipies[recipeId].consumed_item_stacks">
    <v-list-item>
      <v-badge :content="item.quantity" location="right" class="align-start">
        <v-img @error="imagedErrored = true" :src="iconAssetUrlNameRandom(item.item.icon_asset_name).url" height="75" width="75"></v-img>
      </v-badge>
      <v-list-item-title>Name: {{ item.item.name }}</v-list-item-title>
      <template  v-if="recipeInfo.crafted[item.item_type][item.item_id]?.length === 1 || recipeInfo.crafted[item.item_type][item.item_id]?.length === undefined">
        <recusive-crafting-recipe :recipies="[...recipies]" :deepth="deepth+1" v-for="recipe in recipeInfo.crafted[item.item_type][item.item_id]" :recipeId="recipe" :recipeInfo="recipeInfo"></recusive-crafting-recipe>
      </template>
      <template  v-else>
        <v-expansion-panels>
          <v-expansion-panel v-for="recipe in recipeInfo.crafted[item.item_type][item.item_id]"
            :title="recipeInfo.allRecipies[recipe].consumed_item_stacks[0].item.name">
            <v-expansion-panel-text>
              <recusive-crafting-recipe :recipies="[...recipies]" :recipeId="recipe" :recipeInfo="recipeInfo"></recusive-crafting-recipe>
            </v-expansion-panel-text>
          </v-expansion-panel>
      </v-expansion-panels>
    </template>
    </v-list-item>
</template>
</template>