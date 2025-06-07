
<script setup lang="ts">
const props = defineProps<{
  recipeId: number;
  recipeInfo: any;
  recipies: Array<number>

}>();
const exists = props.recipies.findIndex((value) => value == props.recipeId) === -1
props.recipies.push(props.recipeId)
console.log(props.recipies)
</script>
<template >
<template  v-if="exists" v-for = "item in recipeInfo.allRecipies[recipeId].consumed_item_stacks">
    <v-list-item>
      <v-list-item-title>type: {{ recipeInfo.allRecipies[recipeId].building_requirement.building_type }}</v-list-item-title>
      <v-list-item-title>recipeId: {{ recipeId }}</v-list-item-title>
      <v-list-item-title>Quantity: {{ item.quantity }}</v-list-item-title>
      <v-list-item-title> Id: {{ item.item_id }}</v-list-item-title>
      <v-list-item-title> Name: {{ item.item.name }}</v-list-item-title>
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