<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import AutocompleteClaim from "~/components/Bitcraft/autocomplete/AutocompleteClaim.vue";
import AutocompleteUser from "~/components/Bitcraft/autocomplete/AutocompleteUser.vue";
import GetheringShopList from "~/components/Bitcraft/GetheringShopList.vue";
import RecusiveCraftingRecipe from "~/components/Bitcraft/RecusiveCraftingRecipe.vue";
import type { ClaimDescriptionStateWithInventoryAndPlayTime } from "~/types/ClaimDescriptionStateWithInventoryAndPlayTime";
import type { ExtractionRecipeResponse } from "~/types/ExtractionRecipeResponse";
import type { InventorysResponse } from "~/types/InventorysResponse";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";
import {
  buildCraftingGraph,
  buildCraftingPlan,
  buildExtractionEffort,
  changeCraftingSelection,
} from "~/utils/craftingPlan";
import type { InventoryQuantities, RecipeSelections } from "~/utils/craftingPlan";
import { getItemOperationRecipes } from "~/utils/itemOperations";
import { rarityToTextClass, tierToTextClass } from "~/utils";

const playerId = ref<bigint | null>(null);
const claimId = ref<bigint | null | undefined>(null);
const amount = ref<number>(1);
const selections = shallowRef<RecipeSelections>(new Map());
const route = useRoute();

const { data: claimInventoryData, refresh: refreshClaimInventory } =
  await useLazyFetchMsPack<ClaimDescriptionStateWithInventoryAndPlayTime>(
    () => `/api/bitcraft/claims/${claimId.value}`,
    { immediate: false, watch: false },
  );
const { data: playerInventoryData, refresh: refreshPlayerInventory } =
  await useLazyFetchMsPack<InventorysResponse>(
    () => `/api/bitcraft/inventorys/owner_entity_id/${playerId.value}`,
    { immediate: false, watch: false },
  );

watchThrottled(
  () => claimId.value,
  () => {
    if (claimId.value != null) refreshClaimInventory();
    else claimInventoryData.value = undefined;
  },
  { throttle: 50 },
);
watchThrottled(
  () => playerId.value,
  () => {
    if (playerId.value != null) refreshPlayerInventory();
    else playerInventoryData.value = undefined;
  },
  { throttle: 50 },
);

const { data: allRecipiesFetch } = useFetchMsPack<RecipesAllResponse>(() => `/recipes/get_all`);
const { data: extractionRecipesFetch } = useFetchMsPack<ExtractionRecipeResponse[]>(
  () => `/api/bitcraft/extractionRecipes/all`,
);

// Static metadata is shared and never proxied or expanded into alternative trees.
const graph = computed(() =>
  allRecipiesFetch.value ? markRaw(buildCraftingGraph(allRecipiesFetch.value)) : undefined,
);
const extractionEffort = computed(() =>
  buildExtractionEffort(extractionRecipesFetch.value ?? [], allRecipiesFetch.value),
);

const inventoryQuantities = computed(() => {
  const inventory: InventoryQuantities = { Cargo: {}, Item: {} };
  for (const item of claimInventoryData.value?.inventorys?.buildings ?? []) {
    inventory[item.item_type][item.item_id] =
      (inventory[item.item_type][item.item_id] || 0) + (item.quantity || 0);
  }
  for (const inv of playerInventoryData.value?.inventorys ?? []) {
    for (const pocket of inv.pockets) {
      const item = pocket.contents;
      if (!item) continue;
      inventory[item.item_type][item.item_id] =
        (inventory[item.item_type][item.item_id] || 0) + (item.quantity || 0);
    }
  }
  return inventory;
});

const plan = computed(() => {
  const type = route.params.type;
  const id = route.params.id;
  if (
    !graph.value ||
    typeof id !== "string" ||
    !Number.isFinite(Number(id)) ||
    (type !== "Item" && type !== "Cargo")
  )
    return;
  return buildCraftingPlan(graph.value, type, Number(id), amount.value, {
    selections: selections.value,
    inventory: inventoryQuantities.value,
    extraction: extractionEffort.value,
  });
});

const shoppingItems = computed(() => {
  if (!plan.value || !allRecipiesFetch.value) return [];
  const data = allRecipiesFetch.value;
  return (["Item", "Cargo"] as const)
    .flatMap((type) =>
      Object.entries(plan.value!.shoppingList[type]).map(([id, quantity]) => {
        const itemId = Number(id);
        const desc = type === "Item" ? data.item_desc[itemId] : data.cargo_desc[itemId];
        return {
          id: itemId,
          type,
          quantity,
          tier: Number.isFinite(desc?.tier) ? desc!.tier : -Infinity,
          name: desc?.name ?? "",
        };
      }),
    )
    .sort(
      (a, b) =>
        b.tier - a.tier ||
        a.name.localeCompare(b.name) ||
        a.type.localeCompare(b.type) ||
        a.id - b.id,
    );
});

function selectRecipe(path: string, recipe: string | null) {
  selections.value = changeCraftingSelection(selections.value, path, recipe);
}
const operationRecipes = computed(() =>
  plan.value && graph.value
    ? getItemOperationRecipes(graph.value, plan.value.root.type, plan.value.root.id)
    : [],
);
watch(
  () => [route.params.type, route.params.id],
  () => {
    selections.value = new Map();
  },
  { flush: "sync" },
);

const itemDescription = computed(() => {
  const info = plan.value?.root;
  if (!info) return;
  return info.type === "Item"
    ? allRecipiesFetch.value?.item_desc[info.id]
    : allRecipiesFetch.value?.cargo_desc[info.id];
});
const itemHeading = computed(() =>
  itemDescription.value?.name ? `Recipe for ${itemDescription.value.name}` : "Item recipe",
);
const itemSubheading = computed(() =>
  plan.value ? `${plan.value.root.type} #${plan.value.root.id}` : "",
);
useSeoMeta({ title: () => `Recipe Id -> ${route.params.id} Type -> ${route.params.type}` });
</script>

<template>
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-2">
        <p class="text-xs uppercase tracking-[0.3em] text-gray-500 dark:text-gray-400">Crafting</p>
        <div class="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
              {{ itemHeading }}
            </h1>
            <p v-if="itemSubheading" class="text-sm text-gray-600 dark:text-gray-300">
              {{ itemSubheading }}
            </p>
            <div v-if="itemDescription" class="mt-2 flex flex-wrap gap-2">
              <UBadge v-if="itemDescription.tier != null" color="neutral" variant="subtle">
                <span :class="tierToTextClass(itemDescription.tier)"
                  >Tier {{ itemDescription.tier }}</span
                >
              </UBadge>
              <UBadge
                v-if="itemDescription.rarity"
                color="neutral"
                variant="subtle"
                :aria-label="`Rarity: ${itemDescription.rarity}`"
              >
                <span :class="rarityToTextClass(itemDescription.rarity)">{{
                  itemDescription.rarity
                }}</span>
              </UBadge>
            </div>
          </div>
          <div
            class="flex items-center gap-2 rounded-full border border-gray-200 px-3 py-1 text-xs text-gray-600 shadow-sm dark:border-gray-800 dark:text-gray-300"
          >
            <span>Amount</span>
            <span class="font-semibold text-gray-900 dark:text-gray-100">{{
              Intl.NumberFormat().format(amount)
            }}</span>
          </div>
        </div>
      </div>
      <UCard :ui="{ body: 'p-4' }">
        <div class="grid gap-4 lg:grid-cols-3">
          <div class="flex flex-col gap-2">
            <p
              class="text-xs font-semibold uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400"
            >
              Claim inventory
            </p>
            <AutocompleteClaim @model_changed="(item) => (claimId = item)" />
          </div>
          <div class="flex flex-col gap-2">
            <p
              class="text-xs font-semibold uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400"
            >
              Player inventory
            </p>
            <AutocompleteUser @model_changed="(item) => (playerId = item)" />
          </div>
          <div class="flex flex-col gap-2">
            <label
              class="text-xs font-semibold uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400"
              for="amount"
              >Final item amount</label
            >
            <UInput
              id="amount"
              v-model.number="amount"
              type="number"
              min="1"
              placeholder="Enter amount"
            />
          </div>
        </div>
      </UCard>
      <template v-if="plan && graph && allRecipiesFetch">
        <UAlert
          v-if="plan.probabilityAdjusted"
          color="info"
          variant="subtle"
          icon="i-lucide-chart-no-axes-combined"
          title="Average-cost estimate, including rarity chances"
          description="Retries increase the previous-tier items and materials needed throughout the selected tree. Shopping totals are rounded up from average requirements; bad luck can require more. Inventory is deducted from these averages, not from a guaranteed-attempt budget."
        />
        <BitcraftItemOperations
          v-if="operationRecipes.length"
          :key="`${plan.root.type}:${plan.root.id}`"
          :graph="graph"
          :recipes="operationRecipes"
          :data="allRecipiesFetch"
          :type="plan.root.type"
          :id="plan.root.id"
          :inventory="inventoryQuantities"
        />
        <UCard :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-4">
            <div>
              <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">
                Items needed to finish the work
              </h2>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Adjust the claim or player inventory to refine the list.
              </p>
            </div>
            <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
              <GetheringShopList
                v-for="item in shoppingItems"
                :key="`${item.type}-${item.id}`"
                :type="item.type"
                :id="item.id"
                :quantity="item.quantity"
                :item_desc="allRecipiesFetch.item_desc"
                :cargo_desc="allRecipiesFetch.cargo_desc"
              />
            </div>
            <p v-if="!shoppingItems.length" class="text-sm text-muted">
              No additional materials are needed for this plan.
            </p>
          </div>
        </UCard>
        <UCard :ui="{ body: 'p-4' }">
          <div class="flex flex-col gap-4">
            <div>
              <h2 class="text-lg font-semibold text-gray-900 dark:text-gray-100">Recipe tree</h2>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Only the selected plan is calculated. Select another recipe to update quantities and
                total actions.
              </p>
            </div>
            <RecusiveCraftingRecipe
              :item="plan.root"
              :graph="graph"
              :item_desc="allRecipiesFetch.item_desc"
              :cargo_desc="allRecipiesFetch.cargo_desc"
              @select="selectRecipe"
            />
          </div>
        </UCard>
      </template>
      <UEmpty
        v-else
        icon="i-lucide-package-search"
        title="Loading recipe"
        description="Fetching crafting data for this item."
      />
    </div>
  </UContainer>
</template>
