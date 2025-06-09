<script setup lang="ts">
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";
import { watchDebounced, watchThrottled } from "@vueuse/shared";

const player = ref<string | undefined>("");
const playerId = ref<BigInt | null>(null);
const claim = ref<string | undefined>("");
const claimId = ref<BigInt | null>(null);
const amount = ref<number | null>(1);
const route = useRoute();
const router = useRouter();

const {
  public: { api },
} = useRuntimeConfig();

const { data: playerData, refresh: refreshPlayer } = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/players`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};


      if (player.value) {
        options.query.search = player.value;
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length > 2) {
        const query = { player: player.value };
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },
  },
);

const { data: claimInventoryData, refresh: refreshClaimInventory } = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/claims/${claimId.value}`;
  },
  {
    immediate: false,
    onRequest: ({ options }) => {
      options.query = options.query || {};
      options
      if (player.value) {
        options.query.search = player.value;
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length > 2) {
        const query = { player: player.value };
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },

  },
);

const { data: playerInventoryData, refresh: refreshPlayerInventory } = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/inventorys/owner_entity_id/${playerId.value}`;
  },
  {
    immediate: false,
    onRequest: ({ options }) => {
      options.query = options.query || {};
      options
      if (player.value) {
        options.query.search = player.value;
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length > 2) {
        const query = { player: player.value };
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },

  },
);



const { data: claimData, refresh: refreshClaim } = await useLazyFetchMsPack(
  () => {
    return `${api.base}/api/bitcraft/claims`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};


      if (player.value) {
        options.query.search = player.value;
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length > 2) {
        const query = { player: player.value };
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },
  },
);

watchThrottled(
  () => [player.value],
  (value, oldValue) => {
    refreshPlayer();
  },
  { throttle: 50 },
);

watchThrottled(
  () => [claim.value],
  (value, oldValue) => {
    refreshClaim();
  },
  { throttle: 50 },
);
watchThrottled(
  () => [claimId.value],
  (value, oldValue) => {
    if(value[0] === null){
      return
    }
    refreshClaimInventory();
  },
  { throttle: 50 },
);

watchThrottled(
  () => [playerId.value],
  (value, oldValue) => {
    if(value[0] === null){
      return
    }
    refreshPlayerInventory();
  },
  { throttle: 50 },
);

const { data: allRecipiesFetch, pending: allRecipiesPending } =
  useFetchMsPack<RecipesAllResponse>(() => {
    return `${api.base}/recipes/get_all`;
  });

const recipeInfo = computed(() => {
  let hasValues = allRecipiesFetch.value === undefined;
  let allRecipies = allRecipiesFetch.value?.recipes ?? {};
  let cargo_desc = allRecipiesFetch.value?.cargo_desc ?? {};
  let item_desc = allRecipiesFetch.value?.item_desc ?? {};
  let item_list_desc = allRecipiesFetch.value?.item_list_desc ?? {};
  let id = route.params.id;
  let type = route.params.type;
  const consumed = {
    Item: {},
    Cargo: {},
  };
  const crafted = {
    Item: {},
    Cargo: {},
  };
    if (hasValues) {
    return {
      items: [{}],
      consumed,
      crafted,
    };
  }
  
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
          shadow_quantity: Math.max(amount.value / recipe.quantity),
          recipe_quantity: recipe.quantity,
          item_quantity: item.quantity,
          quantity: amount.value,
          children: getConsumedChildren(item.item_id, item.item_type, amount.value, []),
        });
      }
      children.push(itemChildren);
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
          shadow_quantity: quantity,
          recipe_quantity: recipe.quantity,
          item_quantity: item.quantity,
          children: getConsumedChildren(
            item.item_id,
            item.item_type,
            get_qauntity,
            [...recipes],
          ),
        });
      }
      children.push({
        children: itemChildren,
      });
    }
    return children;
  }
  let items = getCraftedChildren();

  const inventory = {
      "Cargo": {},
      "Item": {}
  }
  function combineInvs(inv: any){
    for(const item of inv){
      inventory[item.item_type][item.item_id] = (inventory[item.item_type][item.item_id] || 0) +  (item?.quantity || 0 )
    }
  }
  function combineInvs2(inv: any){
    for(const pockets of inv.pockets){
      if(pockets?.contents?.item_id == undefined){
        continue
      }
      inventory[pockets?.contents?.item_type][pockets?.contents?.item_id] = (inventory[pockets?.contents?.item_type][pockets?.contents?.item_id] || 0) +  (pockets?.contents?.quantity || 0 )
    }
  }
  if(claimInventoryData.value !== undefined && claimInventoryData?.value?.inventorys?.buildings !== undefined){
    combineInvs(claimInventoryData?.value?.inventorys?.buildings)
  }
  if(playerInventoryData.value !== undefined){
    for(const item of playerInventoryData?.value?.inventorys){
      combineInvs2(item)
    }
  }
  if(Object.keys(inventory.Cargo).length !== 0 || Object.keys(inventory.Item).length !== 0){
    function recalcQuantityDeep(item,quantity){
      const quant = getQuantity(item.item_quantity,quantity,item.recipe_quantity)
      item.quantity = quant
      if(item?.children == undefined){
        return
      }
      for(const recipe of item?.children){
        if(recipe?.children == undefined){
          return
        }
        for(const item of recipe?.children){
          recalcQuantityDeep(item,quant)
        }
      }
    }
    function inventoryVSItemList(recipe: any, inventory: any, shadowInventory: any){
      for(const itemIndex in recipe){
        const quantity = (inventory[recipe[itemIndex].type][recipe[itemIndex].id] || 0) - (shadowInventory[recipe[itemIndex].type][recipe[itemIndex].id] || 0)
          if(quantity >= recipe[itemIndex].quantity){
            shadowInventory[recipe[itemIndex].type][recipe[itemIndex].id] = (shadowInventory[recipe[itemIndex].type][recipe[itemIndex].id] || 0) +  recipe[itemIndex].quantity
            recipe[itemIndex].quantity = 0
            recipe.splice(itemIndex, 1)
            continue
          }else{
            shadowInventory[recipe[itemIndex].type][recipe[itemIndex].id] = (shadowInventory[recipe[itemIndex].type][recipe[itemIndex].id] || 0) +  recipe[itemIndex].quantity
            recalcQuantityDeep(recipe[itemIndex],recipe[itemIndex].shadow_quantity - quantity)
          }
        if(recipe[itemIndex]?.children == undefined){
          continue
        }
          for(const recipe2 of recipe[itemIndex].children){
            inventoryVSItemList(recipe2.children,inventory,{...shadowInventory})
        }
      }
    }
    for(const recipe of items){
        inventoryVSItemList(recipe,inventory,{
      "Cargo": {},
      "Item": {}
    })
    }
  }

  return {
    items,
  };
});

useSeoMeta({
  title: "Building Info",
});
</script>

<template>
  <v-container fluid>
     <v-card>
      <v-card-text>
         <v-row>
          <v-col>
            <v-autocomplete
                v-model="claimId"
                v-model:search="claim"
                :items="claimData?.claims || []"
                item-title="name"
                item-value="entity_id"
                label="claim"
                outlined
                dense
                clearable
            ></v-autocomplete>
          </v-col>
          <v-col>
            <v-autocomplete
                v-model="playerId"
                 v-model:search="player"
                :items="playerData?.players || []"
                item-title="username"
                item-value ="entity_id"
                label="player"
                outlined
                dense
                clearable
            ></v-autocomplete>
          </v-col>
          <v-col>
            <v-number-input
            v-model="amount"
            :reverse="false"
            controlVariant="default"
            label="Number of finalized item you want"
            :hideInput="false"
            :inset="false"
          />
          </v-col>
        </v-row>
        <v-list>
          <template v-for="special_items of recipeInfo.items">
              <recusive-crafting-recipe v-if="allRecipiesFetch?.item_desc !== undefined" v-for="item of special_items"
                    :item="item"
                    :item_desc="allRecipiesFetch.item_desc"
                    :cargo_desc="allRecipiesFetch.cargo_desc"
                  />
          </template>
    </v-list>
    </v-card-text>
    </v-card>
  </v-container>
</template>
