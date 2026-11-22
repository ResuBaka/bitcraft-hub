<script setup lang="ts">
import { useMediaQuery } from "@vueuse/core";
import type { ItemDesc } from "~/types/ItemDesc";
import type { CargoDesc } from "~/types/CargoDesc";
import type { CraftingGraph, CraftingPlanNode } from "~/utils/craftingPlan";
import {
  createCraftingTreeView,
  resolveCraftingTreeSelection,
  craftingTreeAncestors,
  pruneCraftingTreeExpanded,
  visibleCraftingTreePaths,
  formatCraftingQuantity,
  formatCraftingChance,
} from "~/utils/craftingTreeView";
import type { CraftingTreeItem } from "~/utils/craftingTreeView";
import { rarityToTextClass, tierToTextClass } from "~/utils";

const props = defineProps<{
  item: CraftingPlanNode;
  graph: CraftingGraph;
  item_desc: { [key: number]: ItemDesc | undefined };
  cargo_desc: { [key: number]: CargoDesc | undefined };
}>();
const emit = defineEmits<{ select: [path: string, recipe: string | null] }>();
const isDesktop = useMediaQuery("(min-width: 1024px)");
const drawerOpen = ref(false);
const selectedPath = ref(props.item.path);
const expandedPaths = ref<string[]>([]);
const tree = useTemplateRef<{ $el: HTMLElement }>("tree");
const rowHeight = 76;
const view = computed(() =>
  createCraftingTreeView(props.item, (type, id) =>
    type === "Item" ? props.item_desc[id] : props.cargo_desc[id],
  ),
);
const selectedItem = computed(
  () => view.value.byPath.get(selectedPath.value) ?? view.value.items[0]!,
);
const virtualize = computed(() =>
  view.value.byPath.size > 150 ? { estimateSize: rowHeight, overscan: 8 } : false,
);

watch(
  view,
  (current, previous) => {
    selectedPath.value = resolveCraftingTreeSelection(current, selectedPath.value);
    if (!previous || previous.items[0]!.path !== current.items[0]!.path) {
      const root = current.items[0]!;
      expandedPaths.value = root.children?.length
        ? [
            root.path,
            ...root.children.filter((child) => child.children?.length).map((child) => child.path),
          ]
        : [];
      drawerOpen.value = false;
    } else expandedPaths.value = pruneCraftingTreeExpanded(current, expandedPaths.value);
  },
  { immediate: true, flush: "sync" },
);
watch(isDesktop, (desktop) => {
  if (desktop) drawerOpen.value = false;
});

function selectTreeItem(item: CraftingTreeItem | undefined) {
  if (item && view.value.byPath.has(item.path)) selectedPath.value = item.path;
}
async function revealSelected() {
  expandedPaths.value = [
    ...new Set([...expandedPaths.value, ...craftingTreeAncestors(view.value, selectedPath.value)]),
  ];
  await nextTick();
  const element = tree.value?.$el;
  if (!element) return;
  if (virtualize.value) {
    const index = visibleCraftingTreePaths(view.value, expandedPaths.value).indexOf(
      selectedPath.value,
    );
    element.scrollTop = Math.max(0, index * rowHeight - element.clientHeight / 2);
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }
  const label = [...element.querySelectorAll<HTMLElement>("[data-plan-path]")].find(
    (element) => element.dataset.planPath === selectedPath.value,
  );
  label?.scrollIntoView({ block: "nearest" });
  if (!drawerOpen.value)
    label?.closest<HTMLElement>("[data-slot=link]")?.focus({ preventScroll: true });
}
function inspectIngredient(path: string) {
  selectedPath.value = resolveCraftingTreeSelection(view.value, path);
  revealSelected();
}
function updateRecipe(path: string, recipe: string | null) {
  emit("select", path, recipe);
  if (recipe !== null) expandedPaths.value = [...new Set([...expandedPaths.value, path])];
}
const getKey = (item: CraftingTreeItem) => item.path;
</script>

<template>
  <div
    class="grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(20rem,24rem)]"
    data-testid="crafting-plan-browser"
  >
    <div class="min-w-0 space-y-3">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="text-xs text-muted">{{ view.byPath.size }} items in the selected plan</p>
        <div class="flex flex-wrap gap-1">
          <UButton
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-unfold-vertical"
            @click="expandedPaths = [...view.expandablePaths]"
            >Expand all</UButton
          >
          <UButton
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-fold-vertical"
            @click="expandedPaths = []"
            >Collapse all</UButton
          >
          <UButton
            size="xs"
            color="neutral"
            variant="ghost"
            icon="i-lucide-scan"
            @click="revealSelected"
            >Reveal selected</UButton
          >
        </div>
      </div>

      <div class="overflow-hidden rounded-xl border border-default bg-default p-1">
        <UTree
          ref="tree"
          v-model:expanded="expandedPaths"
          :model-value="selectedItem"
          :items="view.items"
          :get-key="getKey"
          :default-expanded="[]"
          :nested="false"
          :virtualize="virtualize"
          selection-behavior="replace"
          class="crafting-plan-tree h-[min(65vh,42rem)] min-h-64 overflow-y-auto"
          :ui="{ link: 'h-[76px] min-w-0 px-2! py-2 text-start', item: 'min-w-0' }"
          aria-label="Selected crafting plan"
          @update:model-value="selectTreeItem"
        >
          <template #item="{ item: row, level, expanded }">
            <span
              class="crafting-tree-row flex w-full min-w-0 items-center gap-2"
              :style="{ '--crafting-level': level - 1 }"
              :data-plan-path="row.path"
            >
              <BitcraftInventoryImg
                v-if="row.description"
                :item="row.description"
                :width="32"
                :height="32"
                class="size-8 shrink-0"
                aria-hidden="true"
              />
              <UIcon
                v-else
                name="i-lucide-box"
                class="size-8 shrink-0 text-muted"
                aria-hidden="true"
              />
              <span class="block min-w-0 flex-1">
                <span class="block truncate text-sm font-medium" :title="row.label">{{
                  row.label
                }}</span>
                <span
                  class="flex items-center gap-1.5 overflow-hidden whitespace-nowrap text-[11px] text-muted"
                >
                  <span
                    v-if="row.description?.tier != null"
                    :class="tierToTextClass(row.description.tier)"
                    :title="`Tier ${row.description.tier}`"
                    >T{{ row.description.tier }}</span
                  >
                  <span
                    v-if="row.description?.rarity"
                    :class="rarityToTextClass(row.description.rarity)"
                    >{{ row.description.rarity }}</span
                  >
                  <span
                    v-if="row.node.status === 'craft'"
                    class="text-primary"
                    title="Drop chance including prerequisite crafts, with no retries"
                    >{{ formatCraftingChance(row.node.chainProbability) }}</span
                  >
                  <UIcon
                    v-if="row.node.status === 'inventory'"
                    name="i-lucide-check"
                    class="size-3 text-success"
                    title="Covered by inventory"
                  />
                  <UIcon
                    v-else-if="row.node.status === 'requires-existing'"
                    name="i-lucide-triangle-alert"
                    class="size-3 text-warning"
                    title="Existing item required"
                  />
                  <span v-else-if="row.node.status === 'gather'">Supply</span>
                </span>
              </span>
              <span
                class="shrink-0 rounded-md bg-elevated px-2 py-1 text-xs tabular-nums"
                :title="`Required: ${formatCraftingQuantity(row.node.requested, row.node.expectedDemand)}`"
                >× {{ formatCraftingQuantity(row.node.requested, row.node.expectedDemand) }}</span
              >
              <UIcon
                v-if="row.children?.length"
                name="i-lucide-chevron-right"
                class="size-4 shrink-0 text-muted transition-transform motion-reduce:transition-none"
                :class="expanded ? 'rotate-90' : ''"
                aria-hidden="true"
              />
              <span v-else class="size-4 shrink-0" aria-hidden="true" />
            </span>
          </template>
        </UTree>
      </div>

      <div
        class="flex items-center justify-between gap-2 rounded-lg bg-elevated/50 px-3 py-2 lg:hidden"
      >
        <p class="min-w-0 truncate text-sm">{{ selectedItem.label }}</p>
        <UButton
          size="sm"
          icon="i-lucide-panel-right-open"
          :aria-label="`Open details for ${selectedItem.label}`"
          @click="drawerOpen = true"
          >Details</UButton
        >
      </div>
      <p class="text-xs text-muted">
        Select an item to inspect its recipe, drop chance, inventory, and action estimates. Arrow
        keys navigate the tree.
      </p>
    </div>

    <aside
      v-if="isDesktop"
      class="sticky top-20 max-h-[calc(100vh-7rem)] min-w-0 self-start overflow-y-auto"
      aria-label="Selected item details"
    >
      <UCard :ui="{ body: 'p-4' }">
        <BitcraftCraftingRecipeInspector
          :key="selectedItem.path"
          :item="selectedItem.node"
          :graph="graph"
          :item_desc="item_desc"
          :cargo_desc="cargo_desc"
          @select="updateRecipe"
          @inspect="inspectIngredient"
        />
      </UCard>
    </aside>
    <USlideover
      v-else
      v-model:open="drawerOpen"
      title="Item details"
      :description="selectedItem.label"
    >
      <template #body>
        <BitcraftCraftingRecipeInspector
          :key="selectedItem.path"
          :item="selectedItem.node"
          :graph="graph"
          :item_desc="item_desc"
          :cargo_desc="cargo_desc"
          @select="updateRecipe"
          @inspect="inspectIngredient"
        />
      </template>
    </USlideover>
  </div>
</template>

<style scoped>
.crafting-tree-row {
  padding-inline-start: min(calc(var(--crafting-level) * 0.875rem), 6rem);
}
@media (min-width: 1024px) {
  .crafting-tree-row {
    padding-inline-start: min(calc(var(--crafting-level) * 0.875rem), 10rem);
  }
}
</style>
