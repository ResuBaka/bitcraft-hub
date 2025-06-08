<script setup lang="ts">
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";

const page = ref(1);
const route = useRoute();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}
const {
  public: { api },
} = useRuntimeConfig();

const { data: allRecipiesFetch, pending: allRecipiesPending } =
  useFetchMsPack<RecipesAllResponse>(() => {
    return `${api.base}/recipes/get_all`;
  });

let id = route.params.id;
let type = route.params.type;

const recipeInfo = computed(() => {
  let hasValues = allRecipiesFetch.value === undefined;
  let allRecipies = allRecipiesFetch.value?.recipes ?? {};
  let cargo_desc = allRecipiesFetch.value?.cargo_desc ?? {};
  let item_desc = allRecipiesFetch.value?.item_desc ?? {};
  let item_list_desc = allRecipiesFetch.value?.item_list_desc ?? {};

  const consumed = {
    Item: {},
    Cargo: {},
  };
  const crafted = {
    Item: {},
    Cargo: {},
  };
  function getCraftedItemStack(item_stack, recipie) {
    if (item_stack.item_type == "Item") {
      if (crafted["Item"][item_stack.item_id] == undefined) {
        crafted["Item"][item_stack.item_id] = [];
      }
      crafted["Item"][item_stack.item_id].push({
        id: recipie.id,
        quantity: item_stack.quantity,
      });
      if (
        item_desc[item_stack.item_id].item_list_id !== 0 &&
        item_list_desc[item_desc[item_stack.item_id].item_list_id] !== undefined
      ) {
        for (const possibility of item_list_desc[
          item_desc[item_stack.item_id].item_list_id
        ]?.possibilities) {
          for (const item of possibility.items) {
            getCraftedItemStack(item, recipie);
          }
        }
      }
    } else {
      if (crafted["Cargo"][item_stack.item_id] == undefined) {
        crafted["Cargo"][item_stack.item_id] = [];
      }
      crafted["Cargo"][item_stack.item_id].push({
        id: recipie.id,
        quantity: item_stack.quantity,
      });
    }
  }
  for (const recipie of Object.values(allRecipies)) {
    for (const item_stack of Object.values(recipie.consumed_item_stacks)) {
      if (item_stack.item_type == "Item") {
        if (consumed["Item"][item_stack.item_id] == undefined) {
          consumed["Item"][item_stack.item_id] = [];
        }
        consumed["Item"][item_stack.item_id].push(recipie.id);
      } else {
        if (consumed["Cargo"][item_stack.item_id] == undefined) {
          consumed["Cargo"][item_stack.item_id] = [];
        }
        consumed["Cargo"][item_stack.item_id].push(recipie.id);
      }
    }
    for (const item_stack of Object.values(recipie.crafted_item_stacks)) {
      getCraftedItemStack(item_stack, recipie);
    }
  }

  if (hasValues) {
    return {
      items: [{}],
      consumed,
      crafted,
    };
  }

  let item;
  if (type == "Item") {
    item = item_desc[id];
  } else {
    item = cargo_desc[id];
  }

  function getCraftedChildren() {
    const children = [];
    for (const recipe of crafted[type][id]) {
      let itemChildren = [];

      for (const item of allRecipies[recipe.id]?.crafted_item_stacks) {
        itemChildren.push({
          id: item.item_id,
          type: item.item_type,
          quantity: 1,
          children: getConsumedChildren(item.item_id, item.item_type, 1, []),
        });
      }
      children.push({
        children: itemChildren,
      });
    }
    return children;
  }

  function getQuantity(
    item_quantity: number,
    quantity: number,
    recipe_id_quantity: number,
  ) {
    return Math.ceil((quantity * item_quantity) / recipe_id_quantity);
  }

  function getConsumedChildren(
    id: any,
    type: "Item" | "Cargo",
    quantity: number,
    recipes: number[],
  ) {
    const children = [];
    if (crafted[type][id] == undefined) {
      return;
    }
    for (const recipe of crafted[type][id]) {
      let itemChildren = [];
      const exists = recipes.findIndex((value) => value == recipe.id) !== -1;
      if (exists) {
        continue;
      }
      recipes.push(recipe.id);
      for (const item of allRecipies[recipe.id].consumed_item_stacks) {
        let get_qauntity = getQuantity(
          item.quantity,
          quantity,
          recipe.quantity,
        );
        itemChildren.push({
          id: item.item_id,
          type: item.item_type,
          quantity: get_qauntity,
          children: getConsumedChildren(
            item.item_id,
            item.item_type,
            get_qauntity,
            [...recipes],
          ),
        });
      }
      children.push({
        recipe: true,
        children: itemChildren,
      });
    }
    return children;
  }
  let children = getCraftedChildren();
  let items = [
    {
      id: id,
      type: type,
      quantity: 1,
      children: children,
    },
  ];

  return {
    items,
  };
});

useSeoMeta({
  title: () => `Recipe Id -> ${id} Type -> ${type}`,
});
</script>

<template>
  <v-container fluid>
     <v-card v-if="recipeInfo !== undefined">
      <v-card-text>
        <v-list>
      <recusive-crafting-recipe v-if="allRecipiesFetch?.item_desc !== undefined" v-for="item of recipeInfo.items"
      :item="item"
      :item_desc="allRecipiesFetch.item_desc"
      :cargo_desc="allRecipiesFetch.cargo_desc"
    />
    </v-list>
    </v-card-text>
    </v-card>
  </v-container>
</template>
