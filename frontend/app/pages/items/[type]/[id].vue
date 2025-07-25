<script setup lang="ts">
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";
import GetheringShopList from "~/components/Bitcraft/GetheringShopList.vue";
import AutocompleteClaim from "~/components/Bitcraft/autocomplete/AutocompleteClaim.vue";
import AutocompleteUser from "~/components/Bitcraft/autocomplete/AutocompleteUser.vue";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";
import { watchThrottled } from "@vueuse/shared";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { ClaimDescriptionStateWithInventoryAndPlayTime } from "~/types/ClaimDescriptionStateWithInventoryAndPlayTime";
import type { ItemStack } from "~/types/ItemStack";
import type { CraftingRecipe } from "~/types/CraftingRecipe";
import type { ExpendedRefrence } from "~/types/ExpendedRefrence";
import type { ResolvedInventory } from "~/types/ResolvedInventory";

export type PannelIndexs = {
  pannels: number;
  children: PannelEmptyIndexs[];
};

export type PannelEmptyIndexs = {
  children: PannelIndexs[];
};

const playerId = ref<bigint | null>(null);
const claimId = ref<bigint | null | undefined>(null);
const amount = ref<number>(1);
const pannelIndexs: PannelIndexs[] = reactive([]);
const route = useRoute();

export type objectWithChildren = {
  id: number;
  type: "Cargo" | "Item";
  quantity: number;
  shadow_quantity: number;
  recipe_quantity: number;
  item_quantity: number;
  deleted?: boolean;
  looped?: boolean;
  children: RecipeWithChildren[];
};

export type RecipeWithChildren = {
  children: objectWithChildren[];
};

const { data: claimInventoryData, refresh: refreshClaimInventory } =
  await useLazyFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(
    () => {
      return `/api/bitcraft/claims/${claimId.value}`;
    },
    {
      immediate: false,
    },
  );

const { data: playerInventoryData, refresh: refreshPlayerInventory } =
  await useLazyFetchMsPack<InventorysResponse>(
    () => {
      return `/api/bitcraft/inventorys/owner_entity_id/${playerId.value}`;
    },
    {
      immediate: false,
    },
  );

watchThrottled(
  () => [claimId.value],
  (value, oldValue) => {
    refreshClaimInventory();
  },
  { throttle: 50 },
);

watchThrottled(
  () => [playerId.value],
  (value, oldValue) => {
    refreshPlayerInventory();
  },
  { throttle: 50 },
);

const { data: allRecipiesFetch } = useFetchMsPack<RecipesAllResponse>(() => {
  return `/recipes/get_all`;
});

let id = route.params.id;
let type = route.params.type;

const recipeInfo = computed(() => {
  let hasValues = allRecipiesFetch.value === undefined;
  let allRecipies = allRecipiesFetch.value?.recipes ?? {};
  let cargo_desc = allRecipiesFetch.value?.cargo_desc ?? {};
  let item_desc = allRecipiesFetch.value?.item_desc ?? {};
  let item_list_desc = allRecipiesFetch.value?.item_list_desc ?? {};
  let tempid = route.params.id;
  if (typeof tempid !== "string") {
    return;
  }
  const id: number = +tempid;
  let temptype = route.params.type;
  if (temptype !== "Cargo" && temptype !== "Item") {
    return;
  }
  const type: "Cargo" | "Item" = temptype;
  const consumed: {
    Item: { [key: string]: number[] };
    Cargo: { [key: string]: number[] };
  } = {
    Item: {},
    Cargo: {},
  };
  const crafted: {
    Item: { [key: string]: { id: number; quantity: number }[] };
    Cargo: { [key: string]: { id: number; quantity: number }[] };
  } = {
    Item: {},
    Cargo: {},
  };
  if (hasValues) {
    return;
  }
  function getCraftedItemStack(
    item_stack: ItemStack,
    recipie: CraftingRecipe | undefined,
  ) {
    if (item_stack.item_type == "Item") {
      if (item_stack.item_id === undefined || recipie === undefined) {
        return;
      }
      if (crafted["Item"][item_stack.item_id] === undefined) {
        crafted["Item"][item_stack.item_id] = [];
      }
      const itemSlot = crafted["Item"][item_stack.item_id];
      if (itemSlot === undefined) {
        return;
      }
      itemSlot.push({
        id: recipie.id,
        quantity: item_stack.quantity,
      });
      const itemDesc = item_desc[item_stack.item_id];
      if (itemDesc !== undefined && itemDesc.item_list_id !== 0) {
        const item_list = item_list_desc[itemDesc.item_list_id];
        if (item_list == undefined) {
          return;
        }
        for (const possibility of item_list.possibilities) {
          for (const item of possibility.items) {
            getCraftedItemStack(item, recipie);
          }
        }
      }
    } else {
      if (recipie === undefined) {
        return;
      }
      if (crafted["Cargo"][item_stack.item_id] === undefined) {
        crafted["Cargo"][item_stack.item_id] = [];
      }
      let craftedSlot = crafted["Cargo"][item_stack.item_id];
      if (craftedSlot == undefined) {
        return;
      }
      craftedSlot.push({
        id: recipie.id,
        quantity: item_stack.quantity,
      });
    }
  }
  for (const recipie of Object.values(allRecipies)) {
    if (recipie === undefined) {
      continue;
    }
    for (const item_stack of Object.values(recipie.consumed_item_stacks)) {
      if (item_stack.item_id === undefined) {
        continue;
      }
      if (item_stack.item_type == "Item") {
        let consumedSlot = consumed["Item"];
        if (consumedSlot[item_stack.item_id] == undefined) {
          consumedSlot[item_stack.item_id] = [];
        }
        const itemList = consumedSlot[item_stack.item_id];
        if (itemList === undefined) {
          continue;
        }
        itemList.push(recipie.id);
      } else {
        let consumedSlot = consumed["Cargo"];
        if (consumedSlot[item_stack.item_id] == undefined) {
          consumedSlot[item_stack.item_id] = [];
        }
        const cargoList = consumedSlot[item_stack.item_id];
        if (cargoList === undefined) {
          continue;
        }
        cargoList.push(recipie.id);
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

  function getQuantity(
    item_quantity: number,
    quantity: number,
    recipe_id_quantity: number,
  ) {
    return Math.ceil((quantity * item_quantity) / recipe_id_quantity);
  }

  function getConsumedChildren(
    id: number,
    type: "Item" | "Cargo",
    quantity: number,
    recipes: number[],
  ): RecipeWithChildren[] | undefined | "Loop" {
    const children = [];
    let looped = false;
    if (crafted[type][id] === undefined) {
      return;
    }
    for (const recipe of crafted[type][id]) {
      let itemChildren = [];
      const exists = recipes.findIndex((value) => value == recipe.id) !== -1;
      if (exists) {
        looped = true;
        continue;
      }
      recipes.push(recipe.id);
      const realRecipe = allRecipies[recipe.id];
      if (realRecipe === undefined) {
        continue;
      }
      for (const item of realRecipe.consumed_item_stacks) {
        let get_qauntity = getQuantity(
          item.quantity,
          quantity,
          recipe.quantity,
        );
        let consumedChildren = getConsumedChildren(
          item.item_id,
          item.item_type,
          get_qauntity,
          [...recipes],
        );
        if (consumedChildren === "Loop" || consumedChildren === undefined) {
          itemChildren.push({
            id: item.item_id,
            type: item.item_type,
            looped: consumedChildren === "Loop",
            quantity: get_qauntity,
            shadow_quantity: quantity,
            recipe_quantity: recipe.quantity,
            item_quantity: item.quantity,
            children: [],
          });
        } else {
          itemChildren.push({
            id: item.item_id,
            type: item.item_type,
            quantity: get_qauntity,
            shadow_quantity: quantity,
            recipe_quantity: recipe.quantity,
            item_quantity: item.quantity,
            children: consumedChildren,
          });
        }
      }
      if (itemChildren.filter((item) => !item.looped).length > 0) {
        children.push({
          children: itemChildren,
        });
      }
    }
    if (looped == true && children.length === 0) {
      return "Loop";
    }
    return children;
  }
  const consumedChildren = getConsumedChildren(id, type, amount.value || 1, []);
  if (consumedChildren === "Loop") {
    return;
  }
  let items = {
    id,
    type,
    shadow_quantity: 1,
    recipe_quantity: 1,
    item_quantity: 1,
    quantity: amount.value,
    children: consumedChildren || [],
  };
  const inventory: {
    Cargo: { [key: number]: number };
    Item: { [key: number]: number };
  } = {
    Cargo: {},
    Item: {},
  };
  function combineInvs(inv: ExpendedRefrence[]) {
    for (const item of inv) {
      inventory[item.item_type][item.item_id] =
        (inventory[item.item_type][item.item_id] || 0) + (item?.quantity || 0);
    }
  }
  function combineInvs2(inv: ResolvedInventory) {
    for (const pockets of inv.pockets) {
      if (pockets?.contents?.item_id == undefined) {
        continue;
      }
      inventory[pockets?.contents?.item_type][pockets?.contents?.item_id] =
        (inventory[pockets?.contents?.item_type][pockets?.contents?.item_id] ||
          0) + (pockets?.contents?.quantity || 0);
    }
  }
  if (
    claimInventoryData.value !== undefined &&
    claimInventoryData?.value?.inventorys?.buildings !== undefined
  ) {
    combineInvs(claimInventoryData?.value?.inventorys?.buildings);
  }
  if (playerInventoryData.value !== undefined) {
    for (const item of playerInventoryData.value.inventorys) {
      combineInvs2(item);
    }
  }
  if (
    Object.keys(inventory.Cargo).length !== 0 ||
    Object.keys(inventory.Item).length !== 0
  ) {
    function recalcQuantityDeep(item: objectWithChildren, quantity: number) {
      const itemQuantity = item.item_quantity;
      const itemRecipeQuantity = item.recipe_quantity;
      if (itemQuantity == undefined || itemRecipeQuantity === undefined) {
        return;
      }
      const quant = getQuantity(itemQuantity, quantity, itemRecipeQuantity);
      item.quantity = quant;
      if (item?.children == undefined) {
        return;
      }
      for (const recipe of item.children) {
        if (recipe?.children == undefined) {
          continue;
        }
        for (const item of recipe.children) {
          recalcQuantityDeep(item, quant);
        }
      }
    }
    function inventoryVSItemList(
      recipe: objectWithChildren[],
      inventory: {
        Cargo: {
          [key: number]: number;
        };
        Item: {
          [key: number]: number;
        };
      },
      shadowInventory: {
        Cargo: {
          [key: number]: number;
        };
        Item: {
          [key: number]: number;
        };
      },
    ) {
      for (const itemIndex in recipe) {
        const item = recipe[itemIndex];
        if (item === undefined) {
          continue;
        }
        const type = item.type;
        const id = item.id;
        const itemQuantity = item.quantity;
        const shadow_quantity = item.shadow_quantity;
        if (type !== "Cargo" && type !== "Item") {
          continue;
        }
        if (
          id === undefined ||
          itemQuantity === null ||
          itemQuantity === undefined ||
          shadow_quantity === undefined
        ) {
          continue;
        }
        const quantity =
          (inventory[type][id] || 0) - (shadowInventory[type][id] || 0);
        if (quantity >= itemQuantity) {
          shadowInventory[type][id] =
            (shadowInventory[type][id] || 0) + itemQuantity;
          item.deleted = true;
        } else {
          shadowInventory[type][id] =
            (shadowInventory[type][id] || 0) + itemQuantity;
          recalcQuantityDeep(item, shadow_quantity - quantity);
        }
        if (item?.children == undefined) {
          continue;
        }
        for (const recipe2 of item.children) {
          if (recipe2.children === undefined) {
            continue;
          }
          inventoryVSItemList(recipe2.children, inventory, {
            ...shadowInventory,
          });
        }
      }
    }
    if (items !== undefined) {
      inventoryVSItemList([items], inventory, {
        Cargo: {},
        Item: {},
      });
    }
  }

  function PannelsList(
    recipes: objectWithChildren[],
    pannelIndexs: PannelIndexs[],
  ) {
    if (recipes === undefined) {
      return;
    }
    for (const recipe of recipes) {
      const pannel: PannelIndexs = {
        pannels: 0,
        children: [],
      };
      pannelIndexs.push(pannel);
      if (recipe.children === undefined) {
        continue;
      }
      for (const item of recipe.children) {
        const pannel2: PannelEmptyIndexs = {
          children: [],
        };
        pannel.children.push(pannel2);
        PannelsList(item.children, pannel2.children);
      }
    }
  }
  if (pannelIndexs.length === 0) {
    PannelsList([items], pannelIndexs);
  }

  function ShoppingList(
    recipe: objectWithChildren[],
    list: {
      Cargo: {
        [key: number]: number;
      };
      Item: {
        [key: number]: number;
      };
    },
    pannelIndexs: PannelIndexs[],
  ) {
    for (const itemIndex in recipe) {
      const item = recipe[itemIndex];
      const pannels = pannelIndexs[itemIndex];
      if (item === undefined) {
        continue;
      }
      if (item.deleted === true) {
        if (
          item.id === undefined ||
          item.type === undefined ||
          item.quantity === undefined ||
          item.quantity === null
        ) {
          continue;
        }
        list[item.type][item.id] =
          (list[item.type][item.id] || 0) + item.quantity;
        return;
      }
      if (item.children.length === 0) {
        if (
          item.id === undefined ||
          item.type === undefined ||
          item.quantity === undefined ||
          item.quantity === null
        ) {
          return;
        }
        list[item.type][item.id] =
          (list[item.type][item.id] || 0) + item.quantity;
        continue;
      }
      if (item.type === "Item") {
        const itemDesc = allRecipiesFetch.value.item_desc[item.id];
        if (itemDesc === undefined) {
          continue;
        }
        if (
          itemDesc.name.endsWith(" Animal Hair") ||
          itemDesc.name.endsWith(" Amber Resin")
        ) {
          list[item.type][item.id] =
            (list[item.type][item.id] || 0) + item.quantity;
          continue;
        }
      }
      if (pannels !== undefined) {
        const ItemIndexed = item.children[pannels.pannels];
        const PannelIndexed = pannels.children[pannels.pannels];
        if (ItemIndexed === undefined || PannelIndexed === undefined) {
          return;
        }
        ShoppingList(ItemIndexed.children, list, PannelIndexed.children);
      }
    }
  }
  if (items === undefined) {
    return { items };
  }
  const shoplist: {
    Cargo: {
      [key: number]: number;
    };
    Item: {
      [key: number]: number;
    };
  } = {
    Cargo: {},
    Item: {},
  };
  if (items !== undefined) {
    ShoppingList([items], shoplist, pannelIndexs);
  }
  return {
    items,
    shoplist,
  };
});

useSeoMeta({
  title: () => `Recipe Id -> ${id} Type -> ${type}`,
});
</script>

<template>
  <v-container fluid>
     <v-card>
      <v-card-text>
         <v-row>
          <v-col>
            <autocomplete-claim @model_changed="(item) => claimId=item" />
          </v-col>
          <v-col>
             <autocomplete-user @model_changed="(item) => playerId=item" />
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
          <template v-if="recipeInfo !== undefined && recipeInfo.shoplist !== undefined">
            <v-card-title>Items needed to finish the work</v-card-title>
            <v-row>
              <template v-for="[type,value] of Object.entries(recipeInfo.shoplist)">

                <v-col v-if="type === 'Cargo' || type === 'Item'"  v-for="[id,quantity] of Object.entries(value)" cols="12" sm="6" md="3" lg="2">
                  <gethering-shop-list
                      :type="type"
                      :id="+id"
                      :quantity="quantity"
                      :item_desc="allRecipiesFetch.item_desc"
                      :cargo_desc="allRecipiesFetch.cargo_desc" />
                </v-col>
              </template>
            </v-row>
            <v-divider class="pb-3 mt-3" thickness="5"/>
          </template>
          <template v-if="allRecipiesFetch?.item_desc !== undefined && recipeInfo !== undefined && pannelIndexs[0] !== undefined">
            <v-card-title>Recipe Tree</v-card-title>
            <recusive-crafting-recipe
              :item="recipeInfo.items"
              :item_desc="allRecipiesFetch.item_desc"
              :cargo_desc="allRecipiesFetch.cargo_desc"
              :pannel_indexs="pannelIndexs[0]"
            />
          </template>

        </v-list>
    </v-card-text>
  </v-card> 
</v-container>  
</template>