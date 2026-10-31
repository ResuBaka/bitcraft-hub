<script setup lang="ts">
import type {
  CraftingGraph,
  GraphRecipe,
  InventoryQuantities,
  ItemType,
} from "~/utils/craftingPlan";
import { itemKey } from "~/utils/craftingPlan";
import { buildItemOperation } from "~/utils/itemOperations";
import { estimateDrop } from "~/utils/dropEstimates";
import type { RecipesAllResponse } from "~/types/RecipesAllResponse";

const props = defineProps<{
  graph: CraftingGraph;
  recipes: readonly GraphRecipe[];
  data: RecipesAllResponse;
  type: ItemType;
  id: number;
  inventory: InventoryQuantities;
}>();
const route = useRoute();
const open = ref(route.hash === "#item-operations");
const mode = ref<"recraft" | "scrap">("recraft");
const selectedRecipe = ref<number>();
const runs = ref(1);
const count = new Intl.NumberFormat(undefined, { maximumSignificantDigits: 5 });
const attemptsCount = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
const percent = new Intl.NumberFormat(undefined, { style: "percent", maximumSignificantDigits: 4 });
const highPercent = new Intl.NumberFormat(undefined, {
  style: "percent",
  maximumFractionDigits: 2,
});
function formatChance(value: number, guaranteed = value === 1) {
  const formatted = percent.format(value);
  return !guaranteed && formatted === percent.format(1)
    ? `>${highPercent.format(0.9999)}`
    : formatted;
}
const operations = computed(() => props.recipes.filter((recipe) => recipe.kind === mode.value));
function description(type: ItemType, id: number) {
  return type === "Item" ? props.data.item_desc[id] : props.data.cargo_desc[id];
}
const recipeOptions = computed(() =>
  operations.value.map((recipe, index) => {
    const input = recipe.ingredients[0];
    const item = input ? description(input.item_type, input.item_id) : undefined;
    const name = recipe.name.replace(/\{\d+\}/g, item?.name ?? `#${input?.item_id ?? props.id}`);
    return {
      value: recipe.id,
      label: `${name}${item?.rarity ? ` (${item.rarity})` : ""} — option ${index + 1}`,
    };
  }),
);
const recipeId = computed({
  get: () =>
    operations.value.find((recipe) => recipe.id === selectedRecipe.value)?.id ??
    operations.value[0]?.id,
  set: (id: number | undefined) => {
    selectedRecipe.value = id;
  },
});
const preview = computed(() =>
  recipeId.value === undefined
    ? undefined
    : buildItemOperation(props.graph, props.data, recipeId.value, runs.value, props.inventory),
);
const resultRows = computed(
  () =>
    preview.value?.outputs.map((output) => ({
      ...output,
      estimate: estimateDrop(output.probability, preview.value!.runs),
    })) ?? [],
);
function applyEstimate(attempts: number | null) {
  if (attempts !== null) runs.value = attempts;
}
watch(
  () => [props.type, props.id],
  () => {
    mode.value = props.recipes.some((recipe) => recipe.kind === "recraft") ? "recraft" : "scrap";
    selectedRecipe.value = undefined;
    runs.value = 1;
  },
  { immediate: true },
);
watch(
  () => route.hash,
  (hash) => {
    if (hash === "#item-operations") open.value = true;
  },
);
</script>

<template>
  <UCard id="item-operations" :ui="{ body: 'p-4' }" class="scroll-mt-20">
    <UCollapsible v-model:open="open">
      <UButton
        label="Recraft & scrap existing items"
        icon="i-lucide-recycle"
        color="neutral"
        variant="ghost"
        :trailing-icon="open ? 'i-lucide-chevron-up' : 'i-lucide-chevron-down'"
        block
        class="justify-between"
      />
      <template #content>
        <div class="space-y-4 pt-4">
          <p class="text-sm text-muted">
            Operate on existing items, or find equipment that produces this item when recrafted or
            scrapped. Inputs are never recursively crafted, and this calculation does not change the
            normal plan below. Available counts refer to the selected inventories. No game action is
            performed.
          </p>
          <UTabs
            v-model="mode"
            :items="[
              { label: 'Recraft', value: 'recraft', icon: 'i-lucide-refresh-cw' },
              { label: 'Scrap', value: 'scrap', icon: 'i-lucide-recycle' },
            ]"
            :content="false"
            variant="link"
            aria-label="Existing item operations"
          />

          <UEmpty
            v-if="!operations.length"
            :title="`No ${mode} recipes for this item`"
            description="Choose the other operation, or view another item variant."
          />
          <template v-else>
            <div class="grid gap-4 sm:grid-cols-2">
              <UFormField label="Operation recipe" name="operation-recipe">
                <USelectMenu
                  v-model="recipeId"
                  :items="recipeOptions"
                  value-key="value"
                  class="w-full"
                  aria-label="Operation recipe"
                />
              </UFormField>
              <UFormField label="Recipe runs" name="operation-runs">
                <UInput
                  v-model.number="runs"
                  type="number"
                  min="1"
                  step="1"
                  class="w-full"
                  aria-label="Operation recipe runs"
                />
              </UFormField>
            </div>
            <template v-if="preview">
              <div class="flex flex-wrap gap-2">
                <UBadge color="neutral" variant="subtle"
                  >{{ attemptsCount.format(preview.runs) }} runs</UBadge
                >
                <UBadge color="neutral" variant="subtle"
                  >{{ count.format(preview.actions) }} operation actions</UBadge
                >
              </div>
              <div class="grid gap-4 lg:grid-cols-2">
                <section class="space-y-2" aria-label="Existing items and materials consumed">
                  <h3 class="text-sm font-semibold text-highlighted">
                    Existing items & materials consumed
                  </h3>
                  <div
                    v-for="input in preview.requirements"
                    :key="itemKey(input.type, input.id)"
                    class="flex items-center gap-3 rounded-lg border border-default p-3"
                  >
                    <BitcraftInventoryImg
                      v-if="description(input.type, input.id)"
                      :item="description(input.type, input.id)!"
                      :width="40"
                      :height="40"
                      class="size-10 shrink-0"
                    />
                    <div class="min-w-0 flex-1">
                      <NuxtLink
                        :to="`/items/${input.type}/${input.id}`"
                        class="text-sm font-semibold text-primary hover:underline"
                        >{{
                          description(input.type, input.id)?.name ?? `${input.type} #${input.id}`
                        }}</NuxtLink
                      >
                      <p class="text-xs text-muted">
                        {{ input.existing ? "Existing item" : "Additional material" }} ·
                        {{ description(input.type, input.id)?.rarity }}
                      </p>
                      <p class="text-xs" :class="input.missing ? 'text-warning' : 'text-success'">
                        {{ count.format(input.owned) }} available ·
                        {{ count.format(input.missing) }} missing
                      </p>
                      <p
                        v-if="
                          input.existing &&
                          preview.outputs.some(
                            (output) => output.type === input.type && output.id === input.id,
                          )
                        "
                        class="text-xs text-muted"
                      >
                        Returned by some outcomes and potentially reusable. Counts above are gross
                        uses, not distinct starting items.
                      </p>
                    </div>
                    <UBadge color="neutral" variant="soft">{{
                      count.format(input.quantity)
                    }}</UBadge>
                  </div>
                </section>
                <section class="space-y-2" aria-label="Operation results">
                  <h3 class="text-sm font-semibold text-highlighted">
                    {{ preview.probabilistic ? "Expected results" : "Results" }}
                  </h3>
                  <p v-if="preview.probabilistic" class="text-xs text-muted">
                    Average yields across the outcome probabilities. Actual results can vary.
                  </p>
                  <div
                    v-for="output in resultRows"
                    :key="itemKey(output.type, output.id)"
                    class="flex flex-wrap items-center gap-3 rounded-lg border border-default p-3"
                  >
                    <BitcraftInventoryImg
                      v-if="description(output.type, output.id)"
                      :item="description(output.type, output.id)!"
                      :width="40"
                      :height="40"
                      class="size-10 shrink-0"
                    />
                    <div class="min-w-0 flex-1">
                      <NuxtLink
                        :to="`/items/${output.type}/${output.id}`"
                        class="text-sm font-semibold text-primary hover:underline"
                        >{{
                          description(output.type, output.id)?.name ??
                          `${output.type} #${output.id}`
                        }}</NuxtLink
                      >
                      <p class="text-xs text-muted">
                        {{ description(output.type, output.id)?.rarity }} · {{ output.type }} #{{
                          output.id
                        }}
                      </p>
                    </div>
                    <UBadge color="success" variant="soft">{{
                      count.format(output.quantity)
                    }}</UBadge>
                    <div
                      v-if="preview.probabilistic"
                      class="basis-full space-y-2 border-t border-default pt-2 text-xs"
                    >
                      <template v-if="output.estimate">
                        <p class="font-medium text-highlighted">
                          {{ formatChance(output.estimate.probability) }} chance per attempt
                        </p>
                        <template
                          v-if="output.estimate.probability > 0 && output.estimate.probability < 1"
                        >
                          <p class="text-muted">
                            About {{ count.format(output.estimate.averageAttempts) }} attempts on
                            average to get at least one.
                          </p>
                          <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
                            <div
                              v-for="estimate in [
                                { label: '50%', attempts: output.estimate.attempts50 },
                                { label: '90%', attempts: output.estimate.attempts90 },
                                { label: '95%', attempts: output.estimate.attempts95 },
                                { label: '99%', attempts: output.estimate.attempts99 },
                              ]"
                              :key="estimate.label"
                              class="rounded-md bg-elevated p-2"
                            >
                              <span class="block text-muted">{{ estimate.label }} chance</span>
                              <span class="font-semibold text-highlighted">{{
                                estimate.attempts === null
                                  ? "Unavailable"
                                  : `${attemptsCount.format(estimate.attempts)} ${estimate.attempts === 1 ? "attempt" : "attempts"}`
                              }}</span>
                            </div>
                          </div>
                          <p class="text-muted">
                            Chance of at least one in
                            {{ attemptsCount.format(preview.runs) }}
                            {{ preview.runs === 1 ? "attempt" : "attempts" }}:
                            <strong class="text-highlighted">{{
                              formatChance(
                                output.estimate.chanceAfterAttempts,
                                output.estimate.probability === 1,
                              )
                            }}</strong>
                          </p>
                          <UButton
                            v-if="output.estimate.attempts95 !== null"
                            size="xs"
                            variant="soft"
                            :aria-label="`Use 95% estimate for ${description(output.type, output.id)?.name ?? output.id} (${description(output.type, output.id)?.rarity ?? output.id})`"
                            @click="applyEstimate(output.estimate.attempts95)"
                            >Use 95% estimate ({{
                              attemptsCount.format(output.estimate.attempts95)
                            }}
                            runs)</UButton
                          >
                        </template>
                        <p v-else class="text-muted">
                          {{
                            output.estimate.probability === 1
                              ? "At least one is guaranteed per attempt; quantity may vary."
                              : "This drop cannot occur with the listed odds."
                          }}
                        </p>
                      </template>
                      <p v-else class="text-muted">
                        At-least-one odds are unavailable for this outcome; expected quantities are
                        shown above.
                      </p>
                    </div>
                  </div>
                </section>
              </div>
              <div v-if="preview.probabilistic" class="space-y-2 text-xs text-muted">
                <p>
                  Estimates assume independent attempts with fixed listed odds and no pity mechanic.
                  A 95% chance is not a guarantee: roughly 1 in 20 players may still miss the drop.
                </p>
                <p>
                  Input totals are gross usage across all attempts. Returned items may be reusable,
                  so the existing equipment count is not necessarily the number of separate items
                  you must obtain.
                </p>
                <details>
                  <summary class="cursor-pointer text-primary">
                    How the estimate is calculated
                  </summary>
                  <p class="pt-1">
                    For per-attempt chance p: average attempts = 1 / p; chance after n attempts = 1
                    − (1 − p)ⁿ. Confidence targets count attempts to obtain at least one, not
                    multiple copies.
                  </p>
                </details>
              </div>
              <UAlert
                v-if="preview.kind === 'scrap'"
                color="warning"
                variant="subtle"
                icon="i-lucide-triangle-alert"
                title="Scrapping consumes the existing item."
                description="This is a calculation only; no game action is performed."
              />
            </template>
          </template>
        </div>
      </template>
    </UCollapsible>
  </UCard>
</template>
