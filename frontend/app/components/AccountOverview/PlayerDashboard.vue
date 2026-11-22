<script setup lang="ts">
import BlockGrid from "./BlockGrid.client.vue";
import { blockDragType, type AddBlockRequest } from "~/utils/accountOverviewBlocks";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { PlayerActivitySnapshot } from "~/types/PlayerActivitySnapshot";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";
import { parsePlayerLayout } from "~/utils/accountOverviewLayout";
import type { SharedDashboard, TrackedPlayer } from "~/utils/accountOverviewShare";

const props = defineProps<{
  player: TrackedPlayer;
  addRequest: AddBlockRequest | null;
  editMode: boolean;
  shared?: SharedDashboard;
}>();
defineEmits<{ remove: [] }>();
const blockGrid = ref<InstanceType<typeof BlockGrid> | null>(null);
defineExpose({
  exportSetup() {
    const setup = blockGrid.value?.exportSetup();
    return setup ? { ...props.player, ...setup } : null;
  },
});
const dropActive = ref(false);
const layoutDialogOpen = ref(false);
const layoutText = ref("");
const layoutError = ref("");
const layoutNotice = ref("");
const applyingLayout = ref(false);
function openLayoutDialog() {
  const layout = blockGrid.value?.exportLayout();
  if (!layout) return;
  layoutText.value = JSON.stringify(layout);
  layoutError.value = "";
  layoutNotice.value = "";
  layoutDialogOpen.value = true;
}
async function copyLayout() {
  try {
    await navigator.clipboard.writeText(layoutText.value);
    layoutNotice.value = "Copied to clipboard.";
    layoutError.value = "";
  } catch {
    layoutError.value = "Clipboard unavailable. Select and copy the text manually.";
  }
}
async function applyLayout() {
  const layout = parsePlayerLayout(layoutText.value);
  if (!layout) {
    layoutError.value = "Invalid or unsupported layout. Check the text and try again.";
    return;
  }
  applyingLayout.value = true;
  try {
    if (!(await blockGrid.value?.importLayout(layout))) {
      layoutError.value = "The layout could not be applied. Make sure edit mode is enabled.";
      return;
    }
    layoutDialogOpen.value = false;
  } finally {
    applyingLayout.value = false;
  }
}
const signedIn = ref<boolean | null>(null);
const websocket = useWebsocketStore();
let liveRevision = 0;
let responseRevision = 0;
const requestRevisions = new WeakMap<object, number>();
const { data: activity, refresh } = useLazyFetchMsPack<PlayerActivitySnapshot>(
  () => `/api/bitcraft/players/${encodeURIComponent(props.player.id)}/activity`,
  {
    server: false,
    getCachedData: () => undefined,
    onRequest: ({ options }) => {
      requestRevisions.set(options, liveRevision);
    },
    onResponse: ({ options }) => {
      responseRevision = requestRevisions.get(options) ?? -1;
    },
  },
);
watch(
  activity,
  (value) => {
    if (value?.player_id === props.player.id && responseRevision === liveRevision)
      signedIn.value = value.signed_in;
  },
  { immediate: true },
);
registerWebsocketMessageHandler(
  "PlayerPresenceChanged",
  () => [`player_presence.${props.player.id}`],
  (player) => {
    if (player.player_id !== props.player.id) return;
    liveRevision++;
    signedIn.value = player.signed_in;
  },
);
let awaitingAck = true;
registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (awaitingAck && hasSubscribedTopic(topics, "player_presence", props.player.id)) {
    awaitingAck = false;
    void refresh();
  }
});
websocket.sendMessage({ t: "ListSubscribedTopics" });
watch(
  () => websocket.isConnected,
  (connected) => {
    awaitingAck = true;
    if (connected) void refresh();
  },
);

function onDragOver(event: DragEvent) {
  if (!props.editMode || !event.dataTransfer?.types.includes(blockDragType)) return;
  dropActive.value = true;
  blockGrid.value?.allowDrop(event);
}

function onDragLeave(event: DragEvent) {
  if (
    event.currentTarget instanceof HTMLElement &&
    event.relatedTarget instanceof Node &&
    event.currentTarget.contains(event.relatedTarget)
  )
    return;
  dropActive.value = false;
}

function onDrop(event: DragEvent) {
  dropActive.value = false;
  if (props.editMode) blockGrid.value?.dropBlock(event);
}
watch(
  () => props.editMode,
  (editing) => {
    if (!editing) {
      dropActive.value = false;
      layoutDialogOpen.value = false;
    }
  },
);
</script>

<template>
  <UCard
    class="@container flex h-full min-h-0 flex-col overflow-hidden"
    :class="{ 'ring-2 ring-primary': dropActive }"
    :ui="{ header: 'shrink-0 p-2 sm:px-2', body: 'min-h-0 flex-1 overflow-auto p-2 sm:p-2' }"
    @dragover="onDragOver"
    @dragleave="onDragLeave"
    @drop="onDrop"
  >
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-1">
          <span
            v-if="editMode"
            class="dashboard-drag-handle cursor-grab touch-none text-muted active:cursor-grabbing"
            title="Drag to move dashboard"
            aria-hidden="true"
          >
            <UIcon name="i-lucide-grip-vertical" class="size-5" />
          </span>
          <h2 class="truncate text-base font-semibold">
            <NuxtLink
              :to="`/players/${player.id}`"
              class="hover:underline"
              :class="
                signedIn === null
                  ? undefined
                  : signedIn
                    ? 'text-green-600'
                    : 'text-gray-900 dark:text-gray-100'
              "
              :title="signedIn === null ? 'Status unknown' : signedIn ? 'Online' : 'Offline'"
              >{{ player.name }}</NuxtLink
            >
          </h2>
        </div>
        <div v-if="editMode" class="flex shrink-0 items-center">
          <UButton
            icon="i-lucide-copy"
            color="neutral"
            variant="ghost"
            size="xs"
            :aria-label="`Copy or paste ${player.name}'s block layout`"
            title="Copy or paste block layout"
            @click="openLayoutDialog"
          />
          <UButton
            v-if="editMode"
            icon="i-lucide-x"
            color="neutral"
            variant="ghost"
            size="xs"
            :aria-label="`Remove ${player.name} from players to monitor`"
            @click="$emit('remove')"
          />
        </div>
      </div>
    </template>
    <slot :player="player">
      <ClientOnly>
        <BlockGrid
          ref="blockGrid"
          :player-id="player.id"
          :add-request="addRequest"
          :edit-mode="editMode"
          :shared="shared"
          @dropped="dropActive = false"
        />
      </ClientOnly>
    </slot>
    <UModal v-model:open="layoutDialogOpen">
      <template #content>
        <UCard>
          <template #header>
            <h3 class="text-lg font-semibold">Copy or paste block layout</h3>
          </template>
          <p class="mb-2 text-sm text-muted">
            Copy this text to share it, or paste a layout from another player. Applying replaces
            this player's blocks and their positions and sizes; it does not change the player or box
            size.
          </p>
          <textarea
            v-model="layoutText"
            aria-label="Shareable block layout"
            rows="6"
            spellcheck="false"
            class="w-full rounded border border-default bg-default p-2 font-mono text-xs"
            @input="
              layoutError = '';
              layoutNotice = '';
            "
          />
          <p v-if="layoutError" role="alert" class="mt-2 text-sm text-error">{{ layoutError }}</p>
          <p v-if="layoutNotice" role="status" class="mt-2 text-sm text-success">
            {{ layoutNotice }}
          </p>
          <div class="mt-3 flex flex-wrap justify-end gap-2">
            <UButton
              label="Cancel"
              color="neutral"
              variant="ghost"
              @click="layoutDialogOpen = false"
            />
            <UButton label="Copy text" color="neutral" variant="soft" @click="copyLayout" />
            <UButton label="Apply layout" :loading="applyingLayout" @click="applyLayout" />
          </div>
        </UCard>
      </template>
    </UModal>
  </UCard>
</template>
