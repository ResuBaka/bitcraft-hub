<script setup lang="ts">
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import { computed, ref, watch } from "vue";
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
function getDesc(item: any) {
  let desc;
  if (item.type == "Item") {
    desc = props.item_desc[item.id];
  } else {
    desc = props.cargo_desc[item.id];
  }
  return desc;
}
const desc = computed(() => {
  let desc;
  if (props.item.type == "Item") {
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

const getItemTotalActions = (
  item: objectWithChildren,
  pIndex: PannelIndexs,
): number => {
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

const getRecipeTotalActions = (
  recipe: RecipeWithChildren,
  recipeIdx: number,
): number => {
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

const openedRecipePanels = ref<number[]>([]);
watch(
  openedRecipePanels,
  (v) => {
    const last = v.length > 0 ? v[v.length - 1] : null;
    if (last !== null) props.pannel_indexs.pannels = last;
  },
  { deep: true },
);

const displayActions = computed(() => {
  // Prefer exact totalActions when > 0
  if (totalActions.value && totalActions.value > 0) return totalActions.value;

  // If multiple probability-backed alternatives exist, use weighted avg
  if (hasMultipleAlternatives.value && avgTotalActions.value > 0)
    return avgTotalActions.value;

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
          (it: any) =>
            it.item_id === produced.id && it.item_type === produced.type,
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
    <v-list-item v-if="item.deleted === undefined && desc !== undefined">
        <v-badge :content="displayQuantity" location="right" class="align-start" offset-x="-10">
          <v-list-item-title class="align-content-center">Name: {{ desc.name }}</v-list-item-title>
          <v-img :src="iconAssetUrlNameRandom(desc.icon_asset_name).url" height="75" :width="item.type == 'Item' ? 75 : 128"></v-img>
        </v-badge>
            <div v-if="miningEffort !== undefined" class="text--secondary mt-1">Mining effort (avg attempts): {{ Intl.NumberFormat().format(Math.round(miningEffort)) }} </div>
          <template  v-if="item?.children?.length === 1">
            <v-list-item-subtitle v-if="item.children[0] !== undefined">Actions required: {{ displayActionsString }} </v-list-item-subtitle>
            <div v-if="miningEffort !== undefined" class="text--secondary">Attempts per unit: {{ (miningEffort).toFixed(2) }}</div>
        <template v-if="item.children[0] !== undefined">
                <template v-for="(recipe_item, index) in item.children[0].children" :key="index">
            <recusive-crafting-recipe 
              v-if="getChildPannel(index) !== undefined"
              :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" :item_list_desc="item_list_desc" :resource-effort-map="resourceEffortMap"
              :pannel_indexs="getChildPannel(index)!" ></recusive-crafting-recipe>
          </template>
        </template>
      </template>
      <template  v-else>
        <v-list-item-subtitle v-if="item.children">
          Actions required: {{ displayActionsString }} (Total recipes: {{ item.children.length }})
        </v-list-item-subtitle>
        <div v-if="miningEffort !== undefined" class="text--secondary">Attempts per unit: {{ (miningEffort).toFixed(2) }}</div>
        <v-expansion-panels v-model="openedRecipePanels" multiple>
          <v-expansion-panel v-for="(recipe, index) in item.children" :key="index"
            :title="`[Recipe ${index + 1}] ${getDesc(recipe.children[0]).name} (Total: ${Intl.NumberFormat().format(getRecipeTotalActions(recipe, index))} Step: ${Intl.NumberFormat().format(recipe.shadow_actions_required)})`">
            <v-expansion-panel-text>
                <template v-for="(recipe_item, index2) in recipe.children" :key="index2">
                  <recusive-crafting-recipe 
                    v-if="getChildPannelMulti(index, index2) !== undefined"
                    :item="recipe_item" :item_desc="item_desc" :cargo_desc="cargo_desc" :item_list_desc="item_list_desc" :resource-effort-map="resourceEffortMap"
                    :pannel_indexs="getChildPannelMulti(index, index2)!"  ></recusive-crafting-recipe>
                </template>
            </v-expansion-panel-text>
          </v-expansion-panel>
      </v-expansion-panels>
    </template>
    </v-list-item>
</template>