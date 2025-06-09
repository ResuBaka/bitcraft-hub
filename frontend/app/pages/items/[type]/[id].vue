<script setup lang="ts">
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";
import { watchDebounced, watchThrottled } from "@vueuse/shared";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { PlayersResponse } from "~/types/PlayersResponse";
import type { ClaimDescriptionStateWithInventoryAndPlayTime } from "~/types/ClaimDescriptionStateWithInventoryAndPlayTime";
import type { ClaimResponse } from "~/types/ClaimResponse";
import type { ItemStack } from "~/types/ItemStack";
import type { CraftingRecipe } from "~/types/CraftingRecipe";
import type { ExpendedRefrence } from "~/types/ExpendedRefrence";
import type { ResolvedInventory } from "~/types/ResolvedInventory";

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

type objectWithChildren = {
  id?: number;
  type?: "Cargo" | "Item";
  quantity?: number;
  shadow_quantity?: number;
  recipe_quantity?: number;
  item_quantity?: number;
  children?: objectWithChildren[];
};
const { data: playerData, refresh: refreshPlayer } =
  await useLazyFetchMsPack<PlayersResponse>(
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

const { data: claimInventoryData, refresh: refreshClaimInventory } =
  await useLazyFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(
    () => {
      return `${api.base}/api/bitcraft/claims/${claimId.value}`;
    },
    {
      immediate: false,
      onRequest: ({ options }) => {
        options.query = options.query || {};
        options;
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

const { data: playerInventoryData, refresh: refreshPlayerInventory } =
  await useLazyFetchMsPack<InventorysResponse>(
    () => {
      return `${api.base}/api/bitcraft/inventorys/owner_entity_id/${playerId.value}`;
    },
    {
      immediate: false,
      onRequest: ({ options }) => {
        options.query = options.query || {};
        options;
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

const { data: claimData, refresh: refreshClaim } =
  await useLazyFetchMsPack<ClaimResponse>(
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
    if (value[0] === null) {
      return;
    }
    refreshClaimInventory();
  },
  { throttle: 50 },
);

watchThrottled(
  () => [playerId.value],
  (value, oldValue) => {
    if (value[0] === null) {
      return;
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
    return {
      items: [{}],
      consumed,
      crafted,
    };
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
      const itemSlot =  crafted["Item"][item_stack.item_id]
      if(itemSlot === undefined){
        return
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
      let craftedSlot = crafted["Cargo"][item_stack.item_id]
      if(craftedSlot == undefined){
        return
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
        const itemList = consumedSlot[item_stack.item_id]
        if(itemList === undefined){
          continue
        }
        itemList.push(recipie.id);
      } else {
        let consumedSlot = consumed["Cargo"];
        if (consumedSlot[item_stack.item_id] == undefined) {
          consumedSlot[item_stack.item_id] = [];
        }
        const cargoList =  consumedSlot[item_stack.item_id]
        if(cargoList === undefined){
          continue
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

  function getCraftedChildren(): objectWithChildren[] | undefined  {
    const children = [];
    if (crafted[type][id] === undefined) {
      return;
    }
    for (const recipe of crafted[type][id]) {
      let itemChildren = [];
      const realRecipe = allRecipies[recipe.id];
      if (realRecipe === undefined) {
        continue;
      }
      for (const item of realRecipe.crafted_item_stacks) {
        const amountValue = amount.value;
        if (amountValue === null) {
          continue;
        }
        itemChildren.push({
          id: item.item_id,
          type: item.item_type,
          shadow_quantity: Math.max(amountValue / recipe.quantity),
          recipe_quantity: recipe.quantity,
          item_quantity: item.quantity,
          quantity: amount.value,
          children: getConsumedChildren(
            item.item_id,
            item.item_type,
            amountValue,
            [],
          ),
        });
      }
      children.push({
        children: itemChildren
      });
    }
    console.log(children)
    return children as objectWithChildren[];
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
  ): objectWithChildren[] | undefined {
    const children = [];
    if (crafted[type][id] === undefined) {
      return;
    }
    for (const recipe of crafted[type][id]) {
      let itemChildren = [];
      const exists = recipes.findIndex((value) => value == recipe.id) !== -1;
      if (exists) {
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
    for (const item of playerInventoryData?.value?.inventorys) {
      combineInvs2(item);
    }
  }
  if (
    Object.keys(inventory.Cargo).length !== 0 ||
    Object.keys(inventory.Item).length !== 0
  ) {
    function recalcQuantityDeep(item: objectWithChildren, quantity: number) {
      const itemQuantity = item.item_quantity
      const itemRecipeQuantity = item.recipe_quantity
      if(itemQuantity == undefined || itemRecipeQuantity === undefined){
        return
      }
      const quant = getQuantity(
        itemQuantity,
        quantity,
        itemRecipeQuantity,
      );
      item.quantity = quant;
      if (item?.children == undefined) {
        return;
      }
      for (const recipe of item?.children) {
        if (recipe?.children == undefined) {
          return;
        }
        for (const item of recipe?.children) {
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
        if(typeof(+itemIndex) !== "number") {
          return
        }
        const item = recipe[itemIndex]
        if(item === undefined){
          continue
        }
        const type = item.type;
        const id = item.id;
        const itemQuantity = item.quantity;
        const shadow_quantity = item.shadow_quantity
        if(type !== "Cargo" && type !== "Item"){
          continue
        }
        if( id === undefined || itemQuantity === undefined || shadow_quantity === undefined){
          continue
        }
        const quantity =
          (inventory[type][id] || 0) -
          (shadowInventory[type][id] || 0);
        if (quantity >= itemQuantity) {
          shadowInventory[type][id] =
            (shadowInventory[type][id] ||
              0) + itemQuantity;
          item.quantity = 0;
          recipe.splice(itemIndex, 1);
          continue;
        } else {
          shadowInventory[type][id] =
            (shadowInventory[type][id] ||
              0) + itemQuantity;
          recalcQuantityDeep(
            item,
            shadow_quantity - quantity,
          );
        }
        if (item?.children == undefined) {
          continue;
        }
        for (const recipe2 of item.children) {
          if(recipe2.children === undefined){
            continue
          }
          inventoryVSItemList(recipe2.children, inventory, {
            ...shadowInventory,
          });
        }
      }
    }
    if (items === undefined) {
      return;
    }
    for(item of items){
      inventoryVSItemList(item.children, inventory, {
      Cargo: {},
      Item: {},
    });
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
          <template v-if="recipeInfo !== undefined" v-for="special_items of recipeInfo.items">
              <recusive-crafting-recipe v-if="allRecipiesFetch?.item_desc !== undefined && special_items.children !== undefined" v-for="item of special_items.children"
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
