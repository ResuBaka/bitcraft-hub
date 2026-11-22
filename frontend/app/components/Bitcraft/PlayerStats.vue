<script setup lang="ts">
import type { CharacterStatsSnapshot, ToolPowers } from "~/types/CharacterStatsSnapshot";
import type { ResolvedInventory } from "~/types/ResolvedInventory";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import {
  applyCharacterStatsDelta,
  isNewerCharacterStatsRevision,
} from "~/utils/characterStatsLive";
import {
  characterStatGroups,
  createDetailedStatFormatter,
  professionStatGroups,
} from "~/utils/playerDetailedStats";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";

const props = defineProps<{ playerId: string; toolbelt?: ResolvedInventory }>();
const emit = defineEmits<{ resyncToolbelt: [] }>();
const websocket = useWebsocketStore();
const topic = "player_character_stats";

const { data, status, refresh } = useLazyFetchMsPack<CharacterStatsSnapshot>(
  () => `/api/bitcraft/players/${encodeURIComponent(props.playerId)}/character-stats`,
  { server: false, immediate: false, getCachedData: () => undefined },
);
const { data: toolPowers, refresh: refreshTools } = useLazyFetchMsPack<ToolPowers | null>(
  "/api/bitcraft/tool-powers",
  { server: false },
);

const snapshot = shallowRef<CharacterStatsSnapshot | null>(null);
const live = ref(false);
let baselineValid = false;
let generation = 0;
let resyncing = false;
let dirty = false;
let disposed = false;
let connectionInterrupted = false;

async function syncSnapshot() {
  dirty = true;
  if (resyncing || disposed) return;
  resyncing = true;
  try {
    while (dirty && !disposed) {
      dirty = false;
      const requestedGeneration = generation;
      await refresh({ dedupe: "defer" });
      const incoming = data.value;
      if (
        disposed ||
        requestedGeneration !== generation ||
        status.value !== "success" ||
        incoming?.player_id !== props.playerId
      )
        continue;
      const current = snapshot.value?.revision;
      if (
        baselineValid &&
        current != null &&
        (incoming.revision == null || isNewerCharacterStatsRevision(current, incoming.revision))
      )
        continue;
      snapshot.value = incoming;
      baselineValid = true;
    }
  } finally {
    resyncing = false;
  }
}

function refreshDetails() {
  void syncSnapshot();
  void refreshTools({ dedupe: "defer" });
}

function resetBaseline() {
  generation++;
  baselineValid = false;
  void syncSnapshot();
}

registerWebsocketMessageHandler(
  "PlayerCharacterStatsChanged",
  () => [topic, `${topic}.${props.playerId}`],
  ({ player_id, delta }) => {
    if (player_id !== null && player_id !== props.playerId) return;
    if (!delta) {
      resetBaseline();
      if (player_id === null) void refreshTools({ dedupe: "defer" });
      return;
    }
    const result = applyCharacterStatsDelta(baselineValid ? snapshot.value : null, delta);
    if (result.status === "applied") snapshot.value = result.snapshot;
    else if (result.status === "resync") {
      baselineValid = false;
      void syncSnapshot();
    }
  },
);
registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (live.value || !hasSubscribedTopic(topics, topic, props.playerId)) return;
  live.value = true;
  resetBaseline();
  void refreshTools({ dedupe: "defer" });
  if (connectionInterrupted) {
    connectionInterrupted = false;
    emit("resyncToolbelt");
  }
});
watch(
  () => websocket.isConnected,
  (connected) => {
    if (!connected) {
      connectionInterrupted = true;
      live.value = false;
      resetBaseline();
    }
  },
  { flush: "sync" },
);

// Poll only when the live subscription isn't available.
let timer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  void syncSnapshot();
  websocket.sendMessage({ t: "ListSubscribedTopics" });
  timer = setInterval(() => {
    if ((!live.value || !baselineValid) && document.visibilityState === "visible") {
      void syncSnapshot();
    }
  }, 15_000);
});
onUnmounted(() => {
  disposed = true;
  if (timer) clearInterval(timer);
});

const values = computed(() => snapshot.value?.values);
const sections = computed(() =>
  values.value
    ? [
        {
          title: "Character",
          groups: characterStatGroups(values.value),
          columns: "sm:grid-cols-2",
        },
        {
          title: "Professions",
          groups: professionStatGroups(values.value, props.toolbelt, toolPowers.value),
          columns: "sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4",
        },
      ]
    : [],
);
const formatStat = createDetailedStatFormatter();
</script>

<template>
  <UCard :ui="{ body: 'space-y-4 p-4' }">
    <template #header>
      <h2 class="text-lg font-semibold">Detailed Stats</h2>
    </template>

    <AccountOverviewStatsBlock :player-id="playerId" title="Vitals" />

    <section class="space-y-3">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="text-sm text-muted">
          Stats include this player’s equipment, active buffs, mounts, and knowledge bonuses.
        </p>
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="sm"
          :loading="status === 'pending'"
          aria-label="Refresh detailed stats"
          @click="refreshDetails"
        />
      </div>
      <p v-if="status === 'error'" role="status" class="text-sm text-error">
        Could not refresh character stats. Try again.
      </p>
      <template v-if="sections.length">
        <section v-for="section in sections" :key="section.title" class="space-y-2">
          <h3 class="text-base font-semibold">{{ section.title }}</h3>
          <p v-if="!section.groups.length" class="text-sm text-muted">
            No profession stats have synced for this player.
          </p>
          <p
            v-else-if="section.title === 'Professions' && (!toolPowers || !toolbelt)"
            class="text-sm text-muted"
          >
            Total Power needs the player’s tool belt and synced tool data.
          </p>
          <div class="grid gap-2" :class="section.columns">
            <section
              v-for="group in section.groups"
              :key="group.title"
              class="rounded-lg border border-default p-3"
            >
              <h4 class="mb-2 text-sm font-semibold">{{ group.title }}</h4>
              <dl class="space-y-1 text-sm">
                <div
                  v-for="stat in group.rows"
                  :key="stat.label"
                  class="flex items-baseline justify-between gap-2"
                >
                  <dt :title="stat.description">{{ stat.label }}</dt>
                  <dd class="shrink-0 whitespace-nowrap text-right font-medium tabular-nums">
                    {{ formatStat(stat) }}
                  </dd>
                </div>
              </dl>
            </section>
          </div>
        </section>
      </template>
      <p v-else-if="status === 'pending' || status === 'idle'" class="text-sm text-muted">
        Loading character stats…
      </p>
      <p v-else-if="status === 'success'" class="text-sm text-muted">
        Detailed stats aren’t synced for this player.
      </p>
    </section>
  </UCard>
</template>
