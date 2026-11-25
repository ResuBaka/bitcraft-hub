<script setup lang="ts">
import { refDebounced, useLocalStorage } from "@vueuse/core";
import { formatNumber } from "~/utils/formatNumber";
import DashboardGrid from "~/components/AccountOverview/DashboardGrid.client.vue";
import type { PlayersResponse } from "~/types/PlayersResponse";
import type { AccountOverviewBlockId, AddBlockRequest } from "~/utils/accountOverviewBlocks";
import {
  decodeTrackingSetup,
  encodeTrackingSetup,
  isTrackingSetupHash,
  type TrackedPlayer,
} from "~/utils/accountOverviewShare";

const playerSearch = ref("");
const editMode = ref(false);
const showBlocks = ref(false);
watch(editMode, (editing) => {
  if (!editing) showBlocks.value = false;
});
const mounted = ref(false);
const route = useRouter().currentRoute;
const setupHash = computed(() =>
  mounted.value && isTrackingSetupHash(route.value.hash) ? route.value.hash : "",
);
const sharedSetup = computed(() => (setupHash.value ? decodeTrackingSetup(setupHash.value) : null));
const websocket = useWebsocketStore();
watch(sharedSetup, (setup) => {
  websocket.sharedTrackingEnabled = !!setup;
  if (setupHash.value) editMode.value = false;
});
onBeforeUnmount(() => {
  websocket.sharedTrackingEnabled = false;
});
onMounted(() => {
  mounted.value = true;
});
const playerQuery = refDebounced(playerSearch, 250);
const selectedPlayer = ref<string | null>(null);
const monitoredPlayers = useLocalStorage<TrackedPlayer[]>("bitcraft-hub@monitored-players", [], {
  initOnMounted: true,
  writeDefaults: false,
});
const displayedPlayers = computed(() =>
  setupHash.value ? (sharedSetup.value ?? []) : monitoredPlayers.value,
);
const { preferences: alertPreferences, unseenCount } = provideAccountOverviewAlerts(displayedPlayers);
useSeoMeta({
  title: () => unseenCount.value ? `(${formatNumber(unseenCount.value)}) Account Overview` : "Account Overview",
  description: "Monitor your account's status and crafting activity.",
});
useHead(() => ({
  link: unseenCount.value ? [{ key: "favicon", rel: "icon", href: "/overview-alert.svg" }] : [],
}));
const dashboardGrid = ref<InstanceType<typeof DashboardGrid> | null>(null);
const shareOpen = ref(false);
const shareLink = ref("");
const shareNotice = ref("");
const toast = useToast();

async function copyShareLink() {
  try {
    await navigator.clipboard.writeText(shareLink.value);
    shareNotice.value = "Link copied to clipboard.";
  } catch {
    shareNotice.value = "Clipboard unavailable. Select and copy the link manually.";
  }
}

function shareSetup() {
  const setup = dashboardGrid.value?.exportSetup();
  if (!setup) {
    toast.add({ title: "The dashboard is still loading. Try again shortly." });
    return;
  }
  const hash = encodeTrackingSetup(setup);
  if (!hash) {
    toast.add({
      title: "Could not share this setup",
      description: "Try sharing fewer players or craft filters.",
    });
    return;
  }
  const url = new URL(window.location.href);
  url.search = "";
  url.hash = hash;
  shareLink.value = url.href;
  shareNotice.value = "";
  shareOpen.value = true;
  void copyShareLink();
}

function removePlayer(id: string) {
  monitoredPlayers.value = monitoredPlayers.value.filter((player) => player.id !== id);
}
const addRequest = ref<AddBlockRequest | null>(null);
let addSequence = 0;
function addBlock(playerId: string, blockId: AccountOverviewBlockId) {
  addRequest.value = { playerId, blockId, sequence: ++addSequence };
}

const {
  data: players,
  pending,
  error,
} = await useLazyFetchMsPack<PlayersResponse>(
  () => `/api/bitcraft/players?per_page=20&search=${encodeURIComponent(playerQuery.value)}`,
  { server: false },
);

const playerOptions = computed(() =>
  (players.value?.players ?? []).map((player) => ({
    value: String(player.entity_id),
    label: player.username,
  })),
);

watch(selectedPlayer, (id) => {
  if (!id) return;
  const player = playerOptions.value.find((option) => option.value === id);
  if (player && !monitoredPlayers.value.some((item) => item.id === id)) {
    monitoredPlayers.value.push({ id: player.value, name: player.label });
  }
  selectedPlayer.value = null;
  playerSearch.value = "";
});
</script>

<template>
  <UContainer class="w-full max-w-none py-4">
    <div class="mb-4 flex flex-wrap items-end justify-between gap-4">
      <div>
        <div class="flex flex-wrap items-center gap-x-4">
          <UButton
            to="/tools"
            label="All Tools"
            icon="i-lucide-arrow-left"
            variant="link"
            class="px-0"
          />
          <h1 class="text-2xl font-bold">
            {{ setupHash ? "Shared Account Overview" : "Account Overview" }}
          </h1>
        </div>
        <p v-if="sharedSetup" class="mt-1 text-sm text-muted">
          Live updates are enabled for this view. Your saved dashboard is unchanged.
        </p>
        <p v-else-if="!setupHash" class="mt-1 text-sm text-muted">
          Enable edit mode to arrange dashboards and blocks. Layouts are saved in this browser.
          Enable WebSocket updates for live crafting data.
        </p>
      </div>
      <div class="flex w-full flex-wrap items-end gap-2 sm:w-auto">
        <AccountOverviewAlertSettings v-model="alertPreferences" :players="displayedPlayers" />
        <AccountOverviewAlertHistory />
        <UButton
          v-if="setupHash"
          :to="{ path: route.path }"
          label="My dashboard"
          icon="i-lucide-layout-dashboard"
          variant="soft"
        />
        <template v-else>
          <UFormField label="Add player" class="w-full sm:w-72">
            <UInputMenu
              v-model="selectedPlayer"
              v-model:search-term="playerSearch"
              :items="playerOptions"
              :loading="mounted && pending"
              value-key="value"
              label-key="label"
              icon="i-lucide-search"
              placeholder="Search players"
              ignore-filter
              class="w-full"
            />
          </UFormField>
          <div v-if="editMode" class="relative">
            <UButton
              icon="i-lucide-blocks"
              label="Add blocks"
              color="neutral"
              variant="soft"
              :aria-expanded="showBlocks"
              aria-controls="block-picker"
              @click="showBlocks = !showBlocks"
            />
            <div
              v-if="showBlocks"
              id="block-picker"
              class="absolute right-0 top-full z-50 mt-2 max-h-[min(80dvh,40rem)] w-64 max-w-[90vw] overflow-auto rounded-lg bg-default shadow-lg"
              @keydown.esc.stop="showBlocks = false"
            >
              <AccountOverviewBlockSidebar :players="monitoredPlayers" @add="addBlock" />
            </div>
          </div>
          <UButton
            icon="i-lucide-share-2"
            label="Share setup"
            color="neutral"
            variant="soft"
            :disabled="!monitoredPlayers.length"
            @click="shareSetup"
          />
          <UButton
            :icon="editMode ? 'i-lucide-check' : 'i-lucide-pencil'"
            :label="editMode ? 'Done editing' : 'Edit layout'"
            :aria-pressed="editMode"
            :color="editMode ? 'primary' : 'neutral'"
            variant="soft"
            @click="editMode = !editMode"
          />
        </template>
      </div>
    </div>
    <p v-if="!setupHash && error" class="mb-4 text-sm text-error">
      Could not load players. Please try again.
    </p>

    <div class="grid items-start gap-4">
      <p v-if="setupHash && !sharedSetup" role="alert" class="text-sm text-error">
        This setup link is invalid or unsupported. Ask the sender for a new link.
      </p>
      <ClientOnly v-else-if="displayedPlayers.length">
        <DashboardGrid
          :key="setupHash"
          ref="dashboardGrid"
          :players="displayedPlayers"
          :shared="sharedSetup ?? undefined"
          :add-request="setupHash ? null : addRequest"
          :edit-mode="!setupHash && editMode"
          @remove="removePlayer"
        />
      </ClientOnly>
      <p v-else class="min-h-[calc(100dvh-16rem)] text-sm text-muted">
        No players added yet. Search above to add a player.
      </p>
    </div>
    <UModal v-model:open="shareOpen" title="Share tracking setup">
      <template #body>
        <p class="mb-3 text-sm text-muted">
          Anyone with this link can open these players, layouts, and craft filters with live
          updates. Changes you make to your setup later won’t change this link.
        </p>
        <input
          :value="shareLink"
          readonly
          aria-label="Tracking setup link"
          class="w-full rounded border border-default bg-default p-2 text-sm"
          @focus="($event.target as HTMLInputElement).select()"
        />
        <p v-if="shareNotice" role="status" class="mt-2 text-sm text-muted">{{ shareNotice }}</p>
      </template>
      <template #footer>
        <UButton label="Copy link" icon="i-lucide-copy" @click="copyShareLink" />
      </template>
    </UModal>
  </UContainer>
</template>
