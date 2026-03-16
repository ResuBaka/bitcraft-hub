<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type {
  objectWithChildren,
  PannelIndexs,
  RecipeWithChildren,
} from "~/pages/items/[type]/[id].vue";
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";

export type ResourceEffort = {
  [itemKey: string]: number; // attempts needed per unit
};

const props = defineProps<{
  item: objectWithChildren;
  item_desc: { [key in number]?: ItemDesc };
  cargo_desc: { [key in number]?: CargoDesc };
  item_list_desc?: { [key in number]?: any };
  resourceEffortMap?: { [resourceId: number]: ResourceEffort };
  pannel_indexs: PannelIndexs;
}>();

defineOptions({ name: "RecusiveCraftingRecipe" });
function getDesc(item: objectWithChildren) {
  let desc: ItemDesc | CargoDesc | undefined;
  if (item.type === "Item") {
    desc = props.item_desc[item.id];
  } else {
    desc = props.cargo_desc[item.id];
  }
  return desc;
}
const desc = computed(() => {
  let desc: ItemDesc | CargoDesc | undefined;
  if (props.item.type === "Item") {
    desc = props.item_desc[props.item.id];
  } else {
    desc = props.cargo_desc[props.item.id];
  }
  return desc;
});

function getChildPannel(index: number) {
  return props.pannel_indexs?.children?.[0]?.children?.[index];
}

function getChildPannelMulti(parentIndex: number, childIndex: number) {
  return props.pannel_indexs?.children?.[parentIndex]?.children?.[childIndex];
}

const getItemTotalActions = (item: objectWithChildren, pIndex: PannelIndexs): number => {
  if (!item.children || item.children.length === 0) return 0;

  const recipeIdx = pIndex.pannels;
  const recipe = item.children[recipeIdx];
  if (!recipe) return 0;

  let total = recipe.shadow_actions_required;

  const recipeMeta = pIndex.children[recipeIdx];
  if (recipeMeta && recipe.children) {
    for (let i = 0; i < recipe.children.length; i++) {
      const ingredient = recipe.children[i];
      const ingredientPIndex = recipeMeta.children[i];
      if (ingredient && ingredientPIndex) {
        total += getItemTotalActions(ingredient, ingredientPIndex);
      }
    }
  }

  return total;
};

const getRecipeTotalActions = (recipe: RecipeWithChildren, recipeIdx: number): number => {
  let total = recipe.shadow_actions_required;

  const recipeMeta = props.pannel_indexs.children[recipeIdx];
  if (recipeMeta && recipe.children) {
    for (let i = 0; i < recipe.children.length; i++) {
      const ingredient = recipe.children[i];
      const ingredientPIndex = recipeMeta.children[i];
      if (ingredient && ingredientPIndex) {
        total += getItemTotalActions(ingredient, ingredientPIndex);
      }
    }
  }

  return total;
};

const totalActions = computed(() => {
  return getItemTotalActions(props.item, props.pannel_indexs);
});

const openedRecipePanels = ref<string | undefined>(undefined);
watch(
  openedRecipePanels,
  (v) => {
    if (v !== undefined) props.pannel_indexs.pannels = Number(v);
  },
  { deep: true },
);

const displayActions = computed(() => {
  // Prefer exact totalActions when > 0
  if (totalActions.value && totalActions.value > 0) return totalActions.value;

  // If multiple probability-backed alternatives exist, use weighted avg
  if (hasMultipleAlternatives.value && avgTotalActions.value > 0) return avgTotalActions.value;

  // Fallback: if there are recipes, use the first recipe total or shadow actions
  if (props.item.children && props.item.children.length > 0) {
    const first = props.item.children[0];
    if (first) {
      // compute total for first recipe using current pannel_indexs if possible
      try {
        const tot = getRecipeTotalActions(first, 0);
        if (tot && tot > 0) return tot;
      } catch (e) {
        // ignore
      }
      if (first.shadow_actions_required && first.shadow_actions_required > 0)
        return first.shadow_actions_required;
    }
  }

  return 0;
});

const displayActionsString = computed(() => {
  const v = displayActions.value || 0;
  const formatted = Intl.NumberFormat().format(v);
  return hasMultipleAlternatives.value ? `${formatted}*` : formatted;
});

const hasMultipleAlternatives = computed(() => {
  // Only mark with * when the item is produced from an item list
  // that contains multiple possibilities (i.e. probability-based options).
  if (props.item.type !== "Item") return false;
  const parentDesc = props.item_desc[props.item.id];
  const listId = parentDesc?.item_list_id;
  if (!listId || !props.item_list_desc) return false;
  const list = props.item_list_desc[listId];
  if (!list || !Array.isArray(list.possibilities)) return false;
  return list.possibilities.length > 1;
});

function getProducedItemFromRecipe(recipe: any) {
  return recipe?.children?.[0];
}

function getRecipeProbability(recipe: any): number {
  const produced = getProducedItemFromRecipe(recipe);
  if (!produced) return 1 / (props.item.children?.length || 1);

  if (props.item.type === "Item") {
    const parentDesc = props.item_desc[props.item.id];
    const listId = parentDesc?.item_list_id;
    if (listId && props.item_list_desc && props.item_list_desc[listId]) {
      const list = props.item_list_desc[listId];
      for (const possibility of list.possibilities || []) {
        const found = (possibility.items || []).find(
          (it: any) => it.item_id === produced.id && it.item_type === produced.type,
        );
        if (found) return possibility.probability || 0;
      }
    }
  }

  // fallback: uniform probability
  return 1 / (props.item.children?.length || 1);
}

const avgShadowActions = computed(() => {
  if (!props.item.children || props.item.children.length === 0) return 0;
  let total = 0;
  let weightSum = 0;
  for (const r of props.item.children) {
    const w = getRecipeProbability(r) || 0;
    total += (r.shadow_actions_required || 0) * w;
    weightSum += w;
  }
  if (weightSum === 0) return 0;
  return Math.round(total / weightSum);
});

const avgTotalActions = computed(() => {
  if (!props.item.children || props.item.children.length === 0) return 0;
  let total = 0;
  let weightSum = 0;
  for (let i = 0; i < props.item.children.length; i++) {
    const r = props.item.children[i];
    const w = getRecipeProbability(r) || 0;
    total += getRecipeTotalActions(r, i) * w;
    weightSum += w;
  }
  if (weightSum === 0) return 0;
  return Math.round(total / weightSum);
});

const displayQuantity = computed(() => {
  const q = props.item.quantity ?? 0;
  return hasMultipleAlternatives.value
    ? `${Intl.NumberFormat().format(q)}*`
    : Intl.NumberFormat().format(q);
});

const iconUrl = computed(() => {
  const iconName = desc.value?.icon_asset_name;
  if (!iconName) return null;
  const icon = iconAssetUrlNameRandom(iconName);
  return icon.show ? icon.url : null;
});

const iconLoadError = ref(false);

watch(iconUrl, () => {
  iconLoadError.value = false;
});

const handleIconError = () => {
  iconLoadError.value = true;
};

const recipeAccordionItems = computed(() => {
  if (!props.item.children || props.item.children.length === 0) return [];
  return props.item.children.map((recipe, index) => {
    const produced = getDesc(recipe.children?.[0]);
    const total = Intl.NumberFormat().format(getRecipeTotalActions(recipe, index));
    const step = Intl.NumberFormat().format(recipe.shadow_actions_required || 0);
    return {
      label: `[Recipe ${index + 1}] ${produced?.name ?? "Unknown"} (Total: ${total} Step: ${step})`,
      value: String(index),
      recipe,
      index,
    };
  });
});

function getMiningEffortForItem(itemId: number) {
  // For now, return undefined as we need resource data to calculate this
  // This would need to be calculated based on which resources produce this item
  return undefined;
}

const miningEffort = computed(() => {
  return getMiningEffortForItem(props.item.id);
});
</script>
<template>
  <div v-if="item.deleted !== true && desc" class="flex flex-col gap-3">
    <div
      class="flex items-start gap-3 rounded-lg border border-gray-200 bg-white/70 p-3 shadow-sm dark:border-gray-800 dark:bg-gray-900/40"
    >
      <div
        class="flex h-14 w-14 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
      >
        <img
          v-if="iconUrl && !iconLoadError"
          :src="iconUrl"
          :alt="desc.name"
          class="h-12 w-12 object-contain"
          loading="lazy"
          @error="handleIconError"
        />
        <UIcon v-else name="i-lucide-box" class="h-6 w-6 text-gray-400" />
      </div>
      <div class="flex-1">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="space-y-1">
            <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">{{ desc.name }}</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              Actions required: {{ displayActionsString }}
              <span v-if="item.children?.length > 1" class="text-gray-400">
                (Total recipes: {{ item.children.length }})
              </span>
            </p>
            <p v-if="miningEffort !== undefined" class="text-xs text-gray-500 dark:text-gray-400">
              Mining effort (avg attempts):
              {{ Intl.NumberFormat().format(Math.round(miningEffort)) }}
            </p>
          </div>
          <UBadge color="neutral" variant="soft">
            {{ displayQuantity }}
          </UBadge>
        </div>
      </div>
    </div>

    <div
      v-if="item.children?.length === 1"
      class="border-l border-gray-200 pl-4 dark:border-gray-800"
    >
      <div class="flex flex-col gap-3">
        <template
          v-for="(recipe_item, index) in item.children[0].children"
          :key="`${recipe_item.type}-${recipe_item.id}-${index}`"
        >
          <RecusiveCraftingRecipe
            v-if="getChildPannel(index)"
            :item="recipe_item"
            :item_desc="item_desc"
            :cargo_desc="cargo_desc"
            :item_list_desc="item_list_desc"
            :resource-effort-map="resourceEffortMap"
            :pannel_indexs="getChildPannel(index)!"
          />
        </template>
      </div>
    </div>

    <div
      v-else-if="item.children?.length"
      class="border-l border-gray-200 pl-4 dark:border-gray-800"
    >
      <UAccordion
        v-model="openedRecipePanels"
        type="single"
        :collapsible="false"
        :items="recipeAccordionItems"
        value-key="value"
        label-key="label"
      >
        <template #content="{ item: recipeItem }">
          <div class="flex flex-col gap-3 py-2">
            <template
              v-for="(recipe_item, index2) in recipeItem.recipe?.children || []"
              :key="`${recipe_item.type}-${recipe_item.id}-${index2}`"
            >
              <RecusiveCraftingRecipe
                v-if="getChildPannelMulti(recipeItem.index, index2)"
                :item="recipe_item"
                :item_desc="item_desc"
                :cargo_desc="cargo_desc"
                :item_list_desc="item_list_desc"
                :resource-effort-map="resourceEffortMap"
                :pannel_indexs="getChildPannelMulti(recipeItem.index, index2)!"
              />
            </template>
          </div>
        </template>
      </UAccordion>
    </div>
  </div>
</template>
