<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { iconAssetUrlNameRandom } from "~/composables/iconAssetName";
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";
import { itemKey, estimateCraftingAttempts } from "~/utils/craftingPlan";
import { estimateDrop } from "~/utils/dropEstimates";
import { rarityToTextClass, tierToTextClass } from "~/utils";
import type { CraftingGraph, CraftingPlanNode } from "~/utils/craftingPlan";

const props = defineProps<{
  item: CraftingPlanNode;
  graph: CraftingGraph;
  item_desc: { [key: number]: ItemDesc | undefined };
  cargo_desc: { [key: number]: CargoDesc | undefined };
}>();
const emit = defineEmits<{ select: [path: string, recipe: string | null] }>();
defineOptions({ name: "RecusiveCraftingRecipe" });

const numberFormat = new Intl.NumberFormat();
const percent = new Intl.NumberFormat(undefined, { style: "percent", maximumSignificantDigits: 4 });
const highPercent = new Intl.NumberFormat(undefined, {
  style: "percent",
  maximumFractionDigits: 2,
});
function formatOdds(value: number) {
  const text = percent.format(value);
  return value < 1 && text === percent.format(1) ? `>${highPercent.format(0.9999)}` : text;
}
const dropEstimate = computed(() => estimateDrop(props.item.probability, 0));
const desc = computed(() =>
  props.item.type === "Item" ? props.item_desc[props.item.id] : props.cargo_desc[props.item.id],
);
const graphItem = computed(() => props.graph.items.get(itemKey(props.item.type, props.item.id)));
const outputs = computed(() => graphItem.value?.craftingRecipes ?? []);
const hasOperations = computed(
  () =>
    !!graphItem.value?.operations.length ||
    graphItem.value?.recipes.some(
      (output) => props.graph.recipes.get(output.recipeId)?.kind !== "craft",
    ),
);
const openedRecipePanels = ref<string>();

function openRecipe(value: unknown) {
  openedRecipePanels.value = typeof value === "string" ? value : undefined;
  if (typeof value === "string" && value !== props.item.recipeKey)
    emit("select", props.item.path, value);
}
function supplyDirectly() {
  openedRecipePanels.value = undefined;
  emit("select", props.item.path, null);
}
watch(
  () => props.item.recipeKey,
  (key) => {
    if (openedRecipePanels.value && openedRecipePanels.value !== key)
      openedRecipePanels.value = key ?? undefined;
  },
);

// Alternatives use only graph metadata and direct step costs. Descendant
// quantities and total actions exist exclusively on the currently selected plan.
const recipeAccordionItems = computed(() =>
  outputs.value.map((output, index) => {
    const recipe = props.graph.recipes.get(output.recipeId)!;
    const needed = output.recrafting ? props.item.requested : props.item.quantity;
    const step = numberFormat.format(
      recipe.actions * estimateCraftingAttempts(output, needed, props.item.expectedDemand),
    );
    const selected = props.item.recipeKey === output.key;
    const total = selected
      ? `${props.item.averageEstimate ? "Avg total" : "Total"}: ${numberFormat.format(props.item.totalActions)} `
      : "";
    const name = recipe.name.replace(/\{\d+\}/g, desc.value?.name ?? `#${props.item.id}`);
    return {
      value: output.key,
      label: `[Recipe ${index + 1}] ${name} (${output.probability === null ? "odds unavailable" : `${formatOdds(output.probability)} chance`} · ${total}Step: ${step})`,
      recipe,
      output,
    };
  }),
);

const iconUrl = computed(() => {
  if (!desc.value?.icon_asset_name) return null;
  const icon = iconAssetUrlNameRandom(desc.value.icon_asset_name);
  return icon.show ? icon.url : null;
});
const iconLoadError = ref(false);
watch(iconUrl, () => {
  iconLoadError.value = false;
});
</script>

<template>
  <div class="flex flex-col gap-3">
    <div
      class="flex items-start gap-3 rounded-lg border border-gray-200 bg-white/70 p-3 shadow-sm dark:border-gray-800 dark:bg-gray-900/40"
    >
      <div
        class="flex size-14 shrink-0 items-center justify-center rounded-md border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950"
      >
        <picture v-if="iconUrl && !iconLoadError">
          <source :srcset="`${iconUrl}.jxl`" type="image/jxl" />
          <source :srcset="`${iconUrl}.avif`" type="image/avif" />
          <img
            :src="`${iconUrl}.webp`"
            :alt="desc?.name ?? 'Item icon'"
            class="size-12 object-contain"
            loading="lazy"
            @error="iconLoadError = true"
          />
        </picture>
        <UIcon v-else name="i-lucide-box" class="size-6 text-gray-400" />
      </div>
      <div class="min-w-0 flex-1 space-y-1">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
            {{ desc?.name ?? `${item.type} #${item.id}` }}
          </p>
          <UBadge color="neutral" variant="soft"
            >{{ item.expectedDemand ? "≈ " : "" }}{{ numberFormat.format(item.requested)
            }}{{ graphItem?.probabilistic ? "*" : "" }}</UBadge
          >
        </div>
        <div v-if="desc" class="flex flex-wrap gap-2">
          <UBadge v-if="desc.tier != null" color="neutral" variant="subtle">
            <span :class="tierToTextClass(desc.tier)">Tier {{ desc.tier }}</span>
          </UBadge>
          <UBadge
            v-if="desc.rarity"
            color="neutral"
            variant="subtle"
            :aria-label="`Rarity: ${desc.rarity}`"
          >
            <span :class="rarityToTextClass(desc.rarity)">{{ desc.rarity }}</span>
          </UBadge>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400">
          {{ item.averageEstimate ? "Estimated actions (average)" : "Actions required" }}:
          {{ numberFormat.format(item.totalActions) }}
        </p>
        <div v-if="item.status === 'craft'" class="space-y-1 text-xs text-muted">
          <p v-if="item.probability !== null" class="text-highlighted">
            {{ formatOdds(item.probability) }} chance per craft for this exact item / rarity.
          </p>
          <p v-else>Drop odds unavailable; costs use the listed expected yield.</p>
          <p>
            {{ item.averageEstimate ? "Average crafting attempts" : "Crafting attempts" }}:
            {{ numberFormat.format(item.attempts) }} for
            {{ numberFormat.format(item.quantity) }} still needed.
          </p>
          <p v-if="item.variableYield">
            Variable batch sizes: costs use an average-yield approximation.
          </p>
          <p v-if="item.expectedDemand">
            This requirement includes average retries at the parent stage.
          </p>
          <details
            v-if="
              dropEstimate &&
              item.probability !== null &&
              item.probability > 0 &&
              item.probability < 1
            "
          >
            <summary class="cursor-pointer text-primary">Drop estimates for one item</summary>
            <p class="pt-1">
              {{ numberFormat.format(dropEstimate.averageAttempts) }} attempts on average for one
              drop;
              {{
                dropEstimate.attempts95 === null
                  ? "95% estimate unavailable"
                  : `${numberFormat.format(dropEstimate.attempts95)} attempts for a 95% chance`
              }}.
            </p>
            <p>
              Local odds only, assuming independent attempts. This is not a 95% guarantee for the
              entire crafting chain or for multiple copies.
            </p>
          </details>
        </div>
        <p v-if="item.owned" class="text-xs text-success">
          {{ numberFormat.format(item.owned) }} covered by inventory;
          {{ numberFormat.format(item.quantity) }} still needed.
        </p>
        <p v-if="item.status === 'requires-existing'" class="text-xs text-warning">
          Requires an existing item. This branch cannot produce it without a cycle.
        </p>
        <p v-else-if="item.recrafting && item.status === 'craft'" class="text-xs text-muted">
          Recrafting uses existing equipment as an input.
        </p>
        <p v-if="outputs.length > 1" class="text-xs text-muted">
          {{ outputs.length }} recipes. Totals are calculated only for the selected recipe.
        </p>
        <p v-if="graphItem?.probabilistic" class="text-xs text-muted">
          Rarity-specific drops are estimated separately; other rarities are not substitutes.
        </p>
      </div>
    </div>

    <div v-if="outputs.length > 1" class="border-l border-gray-200 pl-4 dark:border-gray-800">
      <UAccordion
        :model-value="openedRecipePanels"
        type="single"
        :items="recipeAccordionItems"
        value-key="value"
        label-key="label"
        @update:model-value="openRecipe"
      >
        <template #content="{ item: recipeItem }">
          <div v-if="recipeItem.value === item.recipeKey" class="flex flex-col gap-3 py-2">
            <div class="flex flex-wrap gap-2">
              <UBadge
                v-for="(input, index) in recipeItem.recipe.ingredients"
                :key="index"
                color="neutral"
                variant="subtle"
              >
                {{
                  input.item_type === "Item"
                    ? (item_desc[input.item_id]?.name ?? `Item #${input.item_id}`)
                    : (cargo_desc[input.item_id]?.name ?? `Cargo #${input.item_id}`)
                }}
              </UBadge>
            </div>
            <RecusiveCraftingRecipe
              v-for="child in item.children"
              :key="child.path"
              :item="child"
              :graph="graph"
              :item_desc="item_desc"
              :cargo_desc="cargo_desc"
              @select="(path, recipe) => emit('select', path, recipe)"
            />
          </div>
        </template>
      </UAccordion>
    </div>
    <div
      v-else-if="item.children.length"
      class="flex flex-col gap-3 border-l border-gray-200 pl-4 dark:border-gray-800"
    >
      <RecusiveCraftingRecipe
        v-for="child in item.children"
        :key="child.path"
        :item="child"
        :graph="graph"
        :item_desc="item_desc"
        :cargo_desc="cargo_desc"
        @select="(path, recipe) => emit('select', path, recipe)"
      />
    </div>

    <div v-if="outputs.length" class="flex flex-wrap gap-2">
      <UButton
        v-if="item.recipeKey !== null"
        size="xs"
        color="neutral"
        variant="link"
        :aria-label="`Use ${desc?.name ?? item.id} as a base material`"
        @click="supplyDirectly"
        >Use as a base material</UButton
      >
      <UButton
        v-else
        size="xs"
        color="neutral"
        variant="link"
        @click="emit('select', item.path, graphItem?.defaultRecipe ?? outputs[0]!.key)"
        >Choose a crafting recipe</UButton
      >
    </div>
    <NuxtLink
      v-if="hasOperations"
      :to="`/items/${item.type}/${item.id}#item-operations`"
      class="text-xs text-primary hover:underline"
      >Recraft / scrap this item separately</NuxtLink
    >
  </div>
</template>
