<script setup lang="ts">
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";

const page = ref(1);
const route = useRoute();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}
const {
  public: { api },
} = useRuntimeConfig();


const { data: allRecipiesFetch, pending: allRecipiesPending } = useFetchMsPack(
  () => {
    return `${api.base}/recipes/get_all`;
  },
);
let id = computed(() => {
  return route.params.id;
});
let type = computed(() => {
  return route.params.type;
});

const recipeInfo = computed(() => {
let allRecipies = allRecipiesFetch.value?.recipes ?? {}
let cargo_desc = allRecipiesFetch.value?.cargo_desc ?? {}
let item_desc = allRecipiesFetch.value?.item_desc ?? {}
const consumed = {
  Item: {},
  Cargo: {}
}
const crafted = {
  Item: {},
  Cargo: {}
}
for(const recipie of  Object.values(allRecipies) ){
  for(const item_stack  of  Object.values(recipie.consumed_item_stacks)){
   if(item_stack.item_type == "Item"){
    item_stack.item =  item_desc[item_stack.item_id]
    if(consumed["Item"][item_stack.item_id] == undefined){
      consumed["Item"][item_stack.item_id] = []
    }
    consumed["Item"][item_stack.item_id].push(recipie.id)
   }else{
    item_stack.item = cargo_desc[item_stack.item_id]
    if(consumed["Cargo"][item_stack.item_id] == undefined){
      consumed["Cargo"][item_stack.item_id] = []
    }
    consumed["Cargo"][item_stack.item_id].push(recipie.id)
   }
  }
  for(const item_stack  of  Object.values(recipie.crafted_item_stacks)){
    if(item_stack.item_type == "Item"){
      item_stack.item = item_desc[item_stack.item_id]
        if(crafted["Item"][item_stack.item_id] == undefined){
          crafted["Item"][item_stack.item_id] = []
        }
        crafted["Item"][item_stack.item_id].push(recipie.id)
      }else{
        item_stack.item = cargo_desc[item_stack.item_id]
        if(crafted["Cargo"][item_stack.item_id] == undefined){
          crafted["Cargo"][item_stack.item_id] = []
        }
        crafted["Cargo"][item_stack.item_id].push(recipie.id)
      }
    }
  }
  return {
    allRecipies,
    consumed,
    crafted
  }
});

useSeoMeta({
  title: "Building Info",
});
</script>

<template>
  <v-container fluid>
    <v-card v-if="recipeInfo !== undefined">
      <v-card-text>
        <v-list>
          <v-list>
            <v-list-item>
                <v-list-item-title>How to Craft this Item </v-list-item-title>
                <v-list-item v-for="recipe of recipeInfo.crafted[type][id]">

                  <recusive-crafting-recipe :recipies="[]" :recipeId="recipe" :recipeInfo="recipeInfo"></recusive-crafting-recipe>
                </v-list-item>
            </v-list-item>
          </v-list>
        </v-list>
      </v-card-text>
    </v-card>
  </v-container>
</template>
