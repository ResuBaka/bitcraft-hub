
<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";

const props = defineProps<{
  recipeId: {id: number, quantity: number};
  recipeInfo: any;
  recipies: Array<number>;
  quantity: number;
}>();
const exists =
  props.recipies.findIndex((value) => value == props.recipeId.id) === -1;
props.recipies.push(props.recipeId.id);

function getQuantity(item_quantity: number, quantity: number, recipe_id_quantity:number) {
  return Math.ceil((quantity * item_quantity) / recipe_id_quantity)
}
</script>
<template >
<template  v-if="exists" v-for = "item in recipeInfo.allRecipies[recipeId.id].consumed_item_stacks">
    <v-list-item>
      <v-badge :content="getQuantity(item.quantity, quantity,recipeId.quantity)" location="right" class="align-start">
        <v-img @error="imagedErrored = true" :src="iconAssetUrlNameRandom(item.item.icon_asset_name).url" height="75" width="75"></v-img>
      </v-badge>
      <v-list-item-title>Name: {{ item.item.name }}</v-list-item-title>
      <template  v-if="recipeInfo.crafted[item.item_type][item.item_id]?.length === 1 || recipeInfo.crafted[item.item_type][item.item_id]?.length === undefined">
        <recusive-crafting-recipe :quantity="getQuantity(item.quantity, quantity, recipeId.quantity)" :recipies="[...recipies]" v-for="recipe in recipeInfo.crafted[item.item_type][item.item_id]" :recipeId="recipe" :recipeInfo="recipeInfo"></recusive-crafting-recipe>
      </template>
      <template  v-else>
        <v-expansion-panels>
          <v-expansion-panel v-for="recipe in recipeInfo.crafted[item.item_type][item.item_id]"
            :title="recipeInfo.allRecipies[recipe.id].consumed_item_stacks[0].item.name">
            <v-expansion-panel-text>
              <recusive-crafting-recipe :quantity="getQuantity(item.quantity, quantity,recipeId.quantity)" :recipies="[...recipies]" :recipeId="recipe" :recipeInfo="recipeInfo"></recusive-crafting-recipe>
            </v-expansion-panel-text>
          </v-expansion-panel>
      </v-expansion-panels>
    </template>
    </v-list-item>
</template>
</template>