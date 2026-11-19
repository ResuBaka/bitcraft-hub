<script setup lang="ts">
import { useLocalStorage } from "@vueuse/core";
import type { GridStack, GridStackWidget } from "gridstack";
import "gridstack/dist/gridstack.min.css";
import type { AddBlockRequest } from "~/utils/accountOverviewBlocks";
import type { GridPosition } from "~/utils/accountOverviewLayout";
import type { SharedDashboard, TrackedPlayer } from "~/utils/accountOverviewShare";
import PlayerDashboard from "./PlayerDashboard.vue";

const props = defineProps<{
  players: TrackedPlayer[];
  addRequest: AddBlockRequest | null;
  editMode: boolean;
  shared?: SharedDashboard[];
}>();
const emit = defineEmits<{ remove: [id: string] }>();
const gridElement = ref<HTMLElement | null>(null);
const dashboardRefs = useTemplateRef<InstanceType<typeof PlayerDashboard>[]>("dashboards");
const sharedById = computed(() => Object.fromEntries((props.shared ?? []).map((p) => [p.id, p])));
const layouts = props.shared
  ? ref(Object.fromEntries(props.shared.map((p) => [p.id, p.position])))
  : useLocalStorage<Record<string, GridPosition>>(
      "bitcraft-hub@monitor-dashboard-layout",
      {},
      { initOnMounted: true },
    );
let grid: GridStack | undefined;

function readPositions() {
  const positions: Record<string, GridPosition> = {};
  if (!grid) return positions;
  const nodes = grid.save(false, false, undefined, 12) as GridStackWidget[];
  for (const node of nodes) {
    if (node.id != null && node.x != null && node.y != null && node.w != null && node.h != null) {
      positions[node.id] = { x: node.x, y: node.y, w: node.w, h: node.h };
    }
  }
  return positions;
}

function saveLayout() {
  if (grid && !props.shared) layouts.value = readPositions();
}

function exportSetup(): SharedDashboard[] | null {
  if (!grid) return null;
  const positions = readPositions();
  const setups = new Map(
    dashboardRefs.value?.flatMap((dashboard) => {
      const setup = dashboard.exportSetup();
      return setup ? [[setup.id, setup] as const] : [];
    }),
  );
  const dashboards: SharedDashboard[] = [];
  for (const player of props.players) {
    const setup = setups.get(player.id);
    const position = positions[player.id];
    if (!setup || !position) return null;
    dashboards.push({ ...setup, position });
  }
  return dashboards;
}
defineExpose({ exportSetup });

onMounted(async () => {
  const { GridStack } = await import("gridstack");
  await nextTick();
  if (!gridElement.value) return;
  grid = GridStack.init(
    {
      column: 12,
      cellHeight: 80,
      margin: 4,
      handle: ".dashboard-drag-handle",
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

watch(
  () => props.players.map((player) => player.id),
  async (ids, previous) => {
    if (!grid) return;
    for (const id of previous.filter((id) => !ids.includes(id))) {
      const element = Array.from(gridElement.value?.children ?? []).find(
        (child) => child.getAttribute("gs-id") === id,
      );
      if (element) grid.removeWidget(element as HTMLElement, false, false);
      delete layouts.value[id];
    }
    await nextTick();
    if (!grid || !gridElement.value) return;
    for (const element of Array.from(gridElement.value.children)) {
      const id = element.getAttribute("gs-id");
      if (id && !previous.includes(id)) grid.makeWidget(element as HTMLElement);
    }
    saveLayout();
  },
);

onBeforeUnmount(() => {
  grid?.destroy(false);
  grid = undefined;
});
</script>

<template>
  <div ref="gridElement" class="grid-stack min-h-[calc(100dvh-16rem)]">
    <div
      v-for="player in players"
      :key="player.id"
      class="grid-stack-item"
      :gs-id="player.id"
      :gs-x="layouts[player.id]?.x"
      :gs-y="layouts[player.id]?.y"
      :gs-w="layouts[player.id]?.w ?? 4"
      :gs-h="layouts[player.id]?.h ?? 3"
      gs-min-w="3"
      gs-min-h="2"
    >
      <div class="grid-stack-item-content">
        <PlayerDashboard
          ref="dashboards"
          :player="player"
          :add-request="addRequest"
          :edit-mode="editMode"
          :shared="sharedById[player.id]"
          @remove="emit('remove', player.id)"
        />
      </div>
    </div>
  </div>
</template>
