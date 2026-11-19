<script setup lang="ts">
import type { CargoDesc } from "~/types/CargoDesc";
import type { ItemDesc } from "~/types/ItemDesc";
import type { CraftingGraph, CraftingPlanNode } from "~/utils/craftingPlan";
import { itemKey, estimateCraftingAttempts } from "~/utils/craftingPlan";
import { estimateDrop } from "~/utils/dropEstimates";
import { formatCraftingChance, formatCraftingQuantity } from "~/utils/craftingTreeView";
import { rarityToTextClass, tierToTextClass } from "~/utils";

const props = defineProps<{
  item: CraftingPlanNode;
  graph: CraftingGraph;
  item_desc: { [key: number]: ItemDesc | undefined };
  cargo_desc: { [key: number]: CargoDesc | undefined };
}>();
const emit = defineEmits<{
  select: [path: string, recipe: string | null];
  inspect: [path: string];
}>();
function description(type: "Item" | "Cargo", id: number) {
  return type === "Item" ? props.item_desc[id] : props.cargo_desc[id];
}
const desc = computed(() => description(props.item.type, props.item.id));
const graphItem = computed(() => props.graph.items.get(itemKey(props.item.type, props.item.id)));
const outputs = computed(() => graphItem.value?.craftingRecipes ?? []);
const hasOperations = computed(
  () =>
    !!graphItem.value?.operations.length ||
    graphItem.value?.recipes.some(
      (output) => props.graph.recipes.get(output.recipeId)?.kind !== "craft",
    ),
);
const drop = computed(() => estimateDrop(props.item.probability, 0));
const recipeOptions = computed(() => [
  {
    value: "supply",
    label: "Use as a base material",
    description: "Gather, buy, or provide this item instead of crafting it.",
  },
  ...outputs.value.map((output, index) => {
    const recipe = props.graph.recipes.get(output.recipeId)!;
    const step =
      recipe.actions *
      estimateCraftingAttempts(output, props.item.quantity, props.item.expectedDemand);
    const inputs = recipe.ingredients.map((input) => {
      const desc = description(input.item_type, input.item_id);
      return `${desc?.rarity ?? ""} ${desc?.tier != null ? `T${desc.tier}` : ""} ${desc?.name ?? `#${input.item_id}`}`.trim();
    });
    return {
      value: output.key,
      label: `Recipe ${index + 1} · ${formatCraftingChance(output.probability)} chance · ${formatCraftingQuantity(step)} step actions`,
      description: inputs.join(" + ") || "No input materials",
    };
  }),
]);
const recipeChoice = computed({
  get: () => props.item.recipeKey ?? "supply",
  set: (value: string) => {
    const choice = value === "supply" ? null : value;
    if (choice !== props.item.recipeKey) emit("select", props.item.path, choice);
  },
});
</script>

<template>
  <div class="space-y-5" data-testid="crafting-inspector">
    <div class="flex items-start gap-3">
      <BitcraftInventoryImg
        v-if="desc"
        :item="desc"
        :width="48"
        :height="48"
        class="size-12 shrink-0"
      />
      <UIcon v-else name="i-lucide-box" class="size-10 shrink-0 text-muted" />
      <div class="min-w-0 space-y-2">
        <h3 class="font-semibold text-highlighted">
          {{ desc?.name ?? `${item.type} #${item.id}` }}
        </h3>
        <div class="flex flex-wrap gap-1.5">
          <UBadge v-if="desc?.tier != null" color="neutral" variant="subtle"
            ><span :class="tierToTextClass(desc.tier)">Tier {{ desc.tier }}</span></UBadge
          >
          <UBadge
            v-if="desc?.rarity"
            color="neutral"
            variant="subtle"
            :aria-label="`Rarity: ${desc.rarity}`"
            ><span :class="rarityToTextClass(desc.rarity)">{{ desc.rarity }}</span></UBadge
          >
          <UBadge color="neutral" variant="soft">{{ item.type }} #{{ item.id }}</UBadge>
        </div>
      </div>
    </div>

    <UFormField
      v-if="outputs.length"
      label="Crafting recipe"
      name="crafting-recipe"
      description="Only this recipe's selected branch is calculated."
    >
      <USelectMenu
        v-model="recipeChoice"
        :items="recipeOptions"
        value-key="value"
        class="w-full"
        aria-label="Crafting recipe"
      />
    </UFormField>
    <p v-else class="text-sm text-muted">
      {{
        item.status === "inventory"
          ? "Covered by the selected inventories."
          : "Gather, buy, or supply this material directly."
      }}
    </p>

    <dl class="grid grid-cols-2 gap-3 rounded-lg bg-elevated/50 p-3 text-sm">
      <div>
        <dt class="text-xs text-muted">Required{{ item.expectedDemand ? " (average)" : "" }}</dt>
        <dd class="font-semibold tabular-nums">
          {{ formatCraftingQuantity(item.requested, item.expectedDemand) }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">From inventory</dt>
        <dd
          class="font-semibold tabular-nums"
          :class="item.owned ? 'text-success' : 'text-highlighted'"
        >
          {{ formatCraftingQuantity(item.owned) }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">Still needed</dt>
        <dd class="font-semibold tabular-nums">
          {{ formatCraftingQuantity(item.quantity, item.expectedDemand) }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">
          {{ item.averageEstimate ? "Average attempts" : "Crafting attempts" }}
        </dt>
        <dd class="font-semibold tabular-nums">{{ formatCraftingQuantity(item.attempts) }}</dd>
      </div>
      <div>
        <dt class="text-xs text-muted">This step's actions</dt>
        <dd class="font-semibold tabular-nums">
          {{ formatCraftingQuantity(item.actions, item.averageEstimate) }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">
          {{ item.averageEstimate ? "Average total actions" : "Total actions" }}
        </dt>
        <dd class="font-semibold tabular-nums">
          {{ formatCraftingQuantity(item.totalActions, item.averageEstimate) }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">Estimated step XP</dt>
        <dd class="font-semibold tabular-nums">
          {{ item.stepExperienceKnown ? formatCraftingQuantity(item.experience, true) : "Unknown" }}
        </dd>
      </div>
      <div>
        <dt class="text-xs text-muted">
          Estimated total XP{{ item.experienceIncomplete ? " (partial)" : "" }}
        </dt>
        <dd class="font-semibold tabular-nums">
          {{ formatCraftingQuantity(item.totalExperience, true) }}
        </dd>
      </div>
    </dl>
    <p class="text-xs text-muted">
      XP uses the chosen progress per hit for every step, a 100% XP rate, no critical hits, and
      gathering from the lowest-progress source. Gathering assumes full hits. Different tools or
      skills, resource health, rounding, critical hits, bonuses, and actual yields can change XP.
      Supply and unavailable gathering data are excluded from partial totals.
    </p>

    <UAlert
      v-if="item.status === 'requires-existing'"
      color="warning"
      variant="subtle"
      title="Existing item required"
      description="This branch cannot produce the item without a cycle. Supply it directly or choose another recipe."
    />
    <UAlert
      v-else-if="item.status === 'inventory'"
      color="success"
      variant="subtle"
      title="Already covered"
      description="No additional crafting actions or ingredients are required for this occurrence."
    />

    <section
      v-if="item.status === 'craft'"
      class="space-y-2"
      aria-label="Selected item drop estimates"
    >
      <h4 class="text-sm font-semibold text-highlighted">Drop chance</h4>
      <p class="text-sm font-semibold text-primary">
        {{ formatCraftingChance(item.probability) }} per craft for this exact item / rarity
      </p>
      <template
        v-if="drop && item.probability !== null && item.probability > 0 && item.probability < 1"
      >
        <div class="grid grid-cols-2 gap-2 text-sm">
          <div class="rounded-lg border border-default p-2">
            <span class="block text-xs text-muted">One drop, on average</span
            >{{ formatCraftingQuantity(drop.averageAttempts) }} attempts
          </div>
          <div class="rounded-lg border border-default p-2">
            <span class="block text-xs text-muted">95% chance of one drop</span
            >{{
              drop.attempts95 === null
                ? "Unavailable"
                : `${formatCraftingQuantity(drop.attempts95)} attempts`
            }}
          </div>
        </div>
        <p class="text-xs text-muted">
          Local odds only, assuming independent attempts. Not a 95% guarantee for the entire chain
          or for multiple copies.
        </p>
      </template>
      <p v-if="item.expectedDemand" class="text-xs text-muted">
        This requirement includes average retries at the parent stage.
      </p>
      <p v-if="item.variableYield" class="text-xs text-muted">
        Variable batch sizes: costs use an average-yield approximation.
      </p>
      <p v-if="item.averageEstimate" class="text-xs text-muted">
        Rarity odds carry through the selected prerequisites. Other rarities are not substitutes;
        actual costs can be higher or lower.
      </p>
    </section>

    <section v-if="item.children.length" class="space-y-2" aria-label="Selected recipe ingredients">
      <USeparator />
      <h4 class="text-sm font-semibold text-highlighted">Ingredients</h4>
      <UButton
        v-for="child in item.children"
        :key="child.path"
        color="neutral"
        variant="ghost"
        block
        class="justify-between text-start"
        :aria-label="`Inspect ${description(child.type, child.id)?.name ?? child.id}`"
        @click="emit('inspect', child.path)"
      >
        <span class="min-w-0 truncate">{{
          description(child.type, child.id)?.name ?? `${child.type} #${child.id}`
        }}</span>
        <span class="shrink-0 tabular-nums">{{
          formatCraftingQuantity(child.requested, child.expectedDemand)
        }}</span>
        <UIcon name="i-lucide-chevron-right" class="size-4 shrink-0" />
      </UButton>
    </section>

    <div class="flex flex-wrap gap-3 border-t border-default pt-3 text-xs">
      <NuxtLink :to="`/items/${item.type}/${item.id}`" class="text-primary hover:underline"
        >Open item page</NuxtLink
      >
      <NuxtLink
        v-if="hasOperations"
        :to="`/items/${item.type}/${item.id}#item-operations`"
        class="text-primary hover:underline"
        >Recraft / scrap separately</NuxtLink
      >
    </div>
  </div>
</template>
