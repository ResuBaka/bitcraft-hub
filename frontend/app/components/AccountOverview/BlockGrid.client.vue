<script setup lang="ts">
import { useLocalStorage } from "@vueuse/core";
import type { GridItemHTMLElement, GridStack, GridStackWidget } from "gridstack";
import StatsBlock from "./StatsBlock.vue";
import ActivityBlock from "./ActivityBlock.vue";
import ActiveCraftBlock from "./ActiveCraftBlock.vue";
import PassiveCraftBlock from "./PassiveCraftBlock.vue";
import PlayerPlaceables from "../Bitcraft/PlayerPlaceables.vue";
import type { GridPosition, SharedLayout } from "~/utils/accountOverviewLayout";
import type { SharedDashboard } from "~/utils/accountOverviewShare";
import { overviewAlertTypes } from "~/utils/accountOverviewAlerts";
import {
  accountOverviewBlocks,
  blockDropPosition,
  blockDragType,
  isAccountOverviewBlockId,
  type AccountOverviewBlockId,
  type AddBlockRequest,
} from "~/utils/accountOverviewBlocks";

const props = defineProps<{
  playerId: string;
  playerName: string;
  addRequest: AddBlockRequest | null;
  editMode: boolean;
  shared?: SharedDashboard;
}>();
const emit = defineEmits<{ dropped: [] }>();
const blocksWithAlerts = new Set<AccountOverviewBlockId>(
  Object.values(overviewAlertTypes).map((type) => type.block),
);
const components = {
  stats: StatsBlock,
  activity: ActivityBlock,
  "active-crafts": ActiveCraftBlock,
  "passive-crafts": PassiveCraftBlock,
  placeables: PlayerPlaceables,
};
const toast = useToast();
const gridElement = ref<HTMLElement | null>(null);
const enabledBlocks = props.shared
  ? ref(props.shared.layout.blocks.map((block) => block.id))
  : useLocalStorage<AccountOverviewBlockId[]>(
      () => `bitcraft-hub@monitor-blocks:${props.playerId}`,
      accountOverviewBlocks.map((block) => block.id),
      { initOnMounted: true },
    );
const blocks = computed(() =>
  accountOverviewBlocks.filter((block) => enabledBlocks.value.includes(block.id)),
);
const layout = props.shared
  ? ref(Object.fromEntries(props.shared.layout.blocks.map(({ id, ...position }) => [id, position])))
  : useLocalStorage<Record<string, GridPosition>>(
      () => `bitcraft-hub@monitor-block-layout:${props.playerId}`,
      {},
      { initOnMounted: true },
    );
const activeClaims = props.shared
  ? ref([...props.shared.activeClaims])
  : useLocalStorage<string[]>(`bitcraft-hub@craft-claims:Active:${props.playerId}`, [], {
      initOnMounted: true,
    });
const passiveClaims = props.shared
  ? ref([...props.shared.passiveClaims])
  : useLocalStorage<string[]>(`bitcraft-hub@craft-claims:Passive:${props.playerId}`, [], {
      initOnMounted: true,
    });

function blockBindings(id: AccountOverviewBlockId) {
  if (id === "placeables") return { compact: true };
  const filter =
    id === "active-crafts" ? activeClaims : id === "passive-crafts" ? passiveClaims : null;
  return filter
    ? { claims: filter.value, "onUpdate:claims": (value: string[]) => (filter.value = value) }
    : {};
}
let grid: GridStack | undefined;

function itemElement(id: AccountOverviewBlockId): GridItemHTMLElement | undefined {
  return Array.from(gridElement.value?.children ?? []).find(
    (child) => child.getAttribute("gs-id") === id,
  ) as GridItemHTMLElement | undefined;
}

async function addBlock(id: AccountOverviewBlockId, position?: { x: number; y: number }) {
  if (enabledBlocks.value.includes(id)) {
    toast.add({
      title: "Block already added",
      description: "Each player can have one of each block.",
    });
    return;
  }
  enabledBlocks.value = [...enabledBlocks.value, id];
  await nextTick();
  const element = itemElement(id);
  if (grid && element && !element.gridstackNode) {
    const width = 6;
    grid.makeWidget(element, {
      id,
      w: width,
      h: 3,
      minW: 3,
      minH: 2,
      ...(position
        ? { x: Math.max(0, Math.min(12 - width, position.x)), y: Math.max(0, position.y) }
        : { autoPosition: true }),
    });
    saveLayout();
  }
}

function removeBlock(id: AccountOverviewBlockId) {
  const element = itemElement(id);
  if (grid && element?.gridstackNode) grid.removeWidget(element, false, false);
  enabledBlocks.value = enabledBlocks.value.filter((block) => block !== id);
  if (grid) saveLayout();
}

function allowDrop(event: DragEvent) {
  if (!props.editMode || !event.dataTransfer?.types.includes(blockDragType)) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "copy";
}

function dropBlock(event: DragEvent) {
  if (!props.editMode) return;
  const id = event.dataTransfer?.getData(blockDragType) ?? "";
  if (!isAccountOverviewBlockId(id) || !gridElement.value) return;
  event.preventDefault();
  event.stopPropagation();
  emit("dropped");
  const rect = gridElement.value.getBoundingClientRect();
  const position = blockDropPosition(event.clientX, event.clientY, rect);
  void addBlock(id, position);
}

function exportLayout(): SharedLayout | null {
  if (!grid) return null;
  const nodes = grid.save(false, false, undefined, 12) as GridStackWidget[];
  const exported: SharedLayout["blocks"] = [];
  for (const node of nodes) {
    if (
      node.id == null ||
      !isAccountOverviewBlockId(node.id) ||
      node.x == null ||
      node.y == null ||
      node.w == null ||
      node.h == null
    )
      return null;
    exported.push({ id: node.id, x: node.x, y: node.y, w: node.w, h: node.h });
  }
  return { format: "bitcraft-hub-player-layout", version: 1, blocks: exported };
}

async function importLayout(shared: SharedLayout): Promise<boolean> {
  if (!props.editMode || !grid) return false;
  const activeGrid = grid;
  for (const element of Array.from(gridElement.value?.children ?? [])) {
    if ((element as GridItemHTMLElement).gridstackNode)
      activeGrid.removeWidget(element as HTMLElement, false, false);
  }
  layout.value = Object.fromEntries(
    shared.blocks.map(({ id, x, y, w, h }) => [id, { x, y, w, h }]),
  );
  enabledBlocks.value = shared.blocks.map((block) => block.id);
  await nextTick();
  if (grid !== activeGrid) return false;
  activeGrid.batchUpdate();
  for (const block of [...shared.blocks].sort((a, b) => a.y - b.y || a.x - b.x)) {
    const element = itemElement(block.id);
    if (element) activeGrid.makeWidget(element, { ...block, minW: 3, minH: 2 });
  }
  activeGrid.batchUpdate(false);
  saveLayout();
  return true;
}

function exportSetup() {
  const layout = exportLayout();
  if (!layout) return null;
  return {
    layout,
    activeClaims: enabledBlocks.value.includes("active-crafts") ? [...activeClaims.value] : [],
    passiveClaims: enabledBlocks.value.includes("passive-crafts") ? [...passiveClaims.value] : [],
  };
}

defineExpose({ allowDrop, dropBlock, exportLayout, importLayout, exportSetup });

watch(
  () => props.addRequest,
  (request) => {
    if (props.editMode && request?.playerId === props.playerId) void addBlock(request.blockId);
  },
);

function saveLayout() {
  if (!grid) return;
  const nodes = grid.save(false, false, undefined, 12) as GridStackWidget[];
  const next: typeof layout.value = {};
  for (const node of nodes) {
    if (node.id != null && node.x != null && node.y != null && node.w != null && node.h != null) {
      next[node.id] = { x: node.x, y: node.y, w: node.w, h: node.h };
    }
  }
  layout.value = next;
}

onMounted(async () => {
  const { GridStack } = await import("gridstack");
  await nextTick();
  if (!gridElement.value) return;
  grid = GridStack.init(
    {
      column: 12,
      columnOpts: { breakpoints: [] },
      cellHeight: 40,
      margin: 4,
      handle: ".block-drag-handle",
      alwaysShowResizeHandle: true,
      resizable: { handles: "se" },
    },
    gridElement.value,
  );
  if (!props.editMode) grid.disable(false);
  grid.on("dragstop resizestop", saveLayout);
});

watch(
  () => props.editMode,
  (editing) => {
    if (editing) grid?.enable(false);
    else grid?.disable(false);
  },
);

onBeforeUnmount(() => {
  grid?.destroy(false);
  grid = undefined;
});
</script>

<template>
  <div ref="gridElement" class="grid-stack min-h-24" @dragover="allowDrop" @drop="dropBlock">
    <p v-if="!blocks.length" class="pointer-events-none p-3 text-sm text-muted">
      {{
        editMode
          ? "Drop a block here or add one from the sidebar."
          : shared
            ? "No blocks in this shared dashboard."
            : "Enable edit mode to add a block."
      }}
    </p>
    <div
      v-for="(block, index) in blocks"
      :key="block.id"
      class="grid-stack-item"
      :gs-id="block.id"
      :gs-x="layout[block.id]?.x ?? 0"
      :gs-y="layout[block.id]?.y ?? index * 3"
      :gs-w="layout[block.id]?.w ?? 12"
      :gs-h="layout[block.id]?.h ?? 3"
      gs-min-w="3"
      gs-min-h="2"
    >
      <div class="grid-stack-item-content">
        <component
          :is="components[block.id]"
          v-bind="blockBindings(block.id)"
          :player-id="playerId"
          class="h-full"
        >
          <template #handle>
            <span
              v-if="editMode"
              class="block-drag-handle cursor-grab touch-none text-muted active:cursor-grabbing"
              :title="`Drag to move ${block.name}`"
              aria-hidden="true"
            >
              <UIcon name="i-lucide-grip-vertical" class="size-4" />
            </span>
          </template>
          <template #actions>
            <AccountOverviewMuteButton
              v-if="blocksWithAlerts.has(block.id)"
              :player-id="playerId"
              :block-id="block.id"
              :scope-label="`${playerName}'s ${block.name} block`"
            />
            <UButton
              v-if="editMode"
              icon="i-lucide-x"
              size="xs"
              color="neutral"
              variant="ghost"
              :aria-label="`Remove ${block.name} block`"
              @click="removeBlock(block.id)"
            />
          </template>
        </component>
      </div>
    </div>
  </div>
</template>
