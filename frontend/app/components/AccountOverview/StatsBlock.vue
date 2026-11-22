<script setup lang="ts">
import { watchDebounced } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import { formatFoodTimer, meterValue, summarizeFoodBuffs } from "~/utils/monitorBars";
import type { StatsSnapshot } from "~/types/StatsSnapshot";
import { SnapshotRevision } from "~/utils/snapshotRevision";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";
import { statsMessageAction } from "~/utils/statsMessage";

const props = defineProps<{ playerId: string; title?: string }>();
const revisions = new SnapshotRevision();
let responseRevision = revisions.capture();
const requestRevisions = new WeakMap<object, number>();
const data = shallowRef<StatsSnapshot | null>(null);
const {
  data: fetched,
  pending,
  error,
  refresh,
} = useLazyFetchMsPack<StatsSnapshot>(
  () => `/api/bitcraft/players/${encodeURIComponent(props.playerId)}/stats`,
  {
    server: false,
    onRequest: ({ options }) => {
      requestRevisions.set(options, revisions.capture());
    },
    onResponse: ({ options }) => {
      responseRevision = requestRevisions.get(options) ?? -1;
    },
  },
);
const websocket = useWebsocketStore();
watch(
  fetched,
  (snapshot) => {
    if (
      snapshot &&
      snapshot.player_id === props.playerId &&
      revisions.isCurrent(responseRevision)
    ) {
      data.value = snapshot;
    }
  },
  { immediate: true },
);
const receivedAt = ref(Date.now());
const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | undefined;
watch(data, () => {
  receivedAt.value = Date.now();
  now.value = receivedAt.value;
});
onMounted(() => {
  timer = setInterval(() => {
    now.value = Date.now();
  }, 1000);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

const foodSummary = computed(() =>
  summarizeFoodBuffs(
    (data.value?.buffs ?? []).map((buff) => ({
      ...buff,
      is_food_associated: buff.is_food_associated ?? null,
    })),
    Math.max(0, Math.floor((now.value - receivedAt.value) / 1000)),
  ),
);
const changes = ref(0);
let refreshing = false;
let dirty = false;
async function refreshStats() {
  dirty = true;
  if (refreshing) return;
  refreshing = true;
  try {
    while (dirty) {
      dirty = false;
      // Let an initial or earlier snapshot finish, then request a fresh one.
      if (pending.value) await refresh({ dedupe: "defer" });
      await refresh({ dedupe: "defer" });
    }
  } finally {
    refreshing = false;
  }
}
registerWebsocketMessageHandler(
  "PlayerStatsChanged",
  () => ["player_stats", `player_stats.${props.playerId}`],
  (message) => {
    const action = statsMessageAction(props.playerId, message);
    if (action.type === "ignore") return;
    revisions.invalidate();
    if (action.type === "snapshot") {
      data.value = action.snapshot;
    } else {
      // Global invalidation, or an older server without snapshot payloads.
      changes.value++;
    }
  },
);
let lastSignedIn: boolean | undefined;
registerWebsocketMessageHandler(
  "PlayerPresenceChanged",
  () => [`player_presence.${props.playerId}`],
  (player) => {
    if (player.player_id !== props.playerId) return;
    if (lastSignedIn !== player.signed_in) {
      lastSignedIn = player.signed_in;
      revisions.invalidate();
      changes.value++;
    }
  },
);
watchDebounced(changes, () => void refreshStats(), { debounce: 400, maxWait: 2000 });
let awaitingAck = true;
registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (awaitingAck && hasSubscribedTopic(topics, "player_stats", props.playerId)) {
    awaitingAck = false;
    revisions.invalidate();
    changes.value++;
  }
});
websocket.sendMessage({ t: "ListSubscribedTopics" });
watch(
  () => websocket.isConnected,
  (connected) => {
    awaitingAck = true;
    if (connected) changes.value++;
  },
);
</script>

<template>
  <section
    class="@container flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default p-1.5"
  >
    <div class="flex shrink-0 items-center justify-between gap-1 overflow-x-auto">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <h3 class="truncate text-base font-semibold">{{ title ?? "Stats" }}</h3>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :loading="pending"
          aria-label="Refresh stats"
          @click="refreshStats"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <p v-if="error" class="mt-1 text-sm text-error">Could not load stats. Refresh to retry.</p>
      <p v-if="pending && !data" class="mt-1 text-sm text-muted">Loading…</p>
      <template v-else-if="data">
        <p v-if="!data.available" class="mt-1 text-sm text-muted">
          Stats are not synced for this region.
        </p>
        <div v-else class="grid grid-cols-1 gap-x-2 gap-y-1 @md:grid-cols-2">
          <AccountOverviewStatBar
            label="Health"
            :value="data.health"
            :max="data.max_health"
            color="health"
          />
          <AccountOverviewStatBar
            label="Stamina"
            :value="data.stamina"
            :max="data.max_stamina"
            color="stamina"
          />
          <div
            class="min-w-0 text-sm @md:col-span-2"
            title="Food-associated buffs may also be granted by other sources."
          >
            <div class="flex flex-wrap justify-between gap-x-2">
              <span class="font-medium">Food</span>
              <span class="text-muted">
                <template v-if="data.buffs == null || !data.food_metadata_available"
                  >Unavailable</template
                >
                <template v-else-if="!foodSummary.count">{{
                  data.buffs_complete ? "None active" : "Timing unavailable"
                }}</template>
                <template v-else-if="foodSummary.next?.remaining == null">Permanent</template>
                <template v-else>
                  {{ formatFoodTimer(foodSummary.next.remaining) }} remaining{{
                    foodSummary.next.paused ? " (paused)" : ""
                  }}{{ foodSummary.count > 1 ? " · next expiry" : "" }}
                </template>
              </span>
            </div>
            <progress
              v-if="
                data.food_metadata_available &&
                foodSummary.next &&
                meterValue(foodSummary.next.remaining, foodSummary.next.duration) !== null
              "
              :value="meterValue(foodSummary.next.remaining, foodSummary.next.duration)!"
              :max="foodSummary.next.duration"
              aria-label="Food buff time remaining"
              class="mt-0.5 block h-1.5 w-full accent-green-500"
            />
            <p v-if="!data.buffs_complete && foodSummary.count" class="mt-1 text-xs text-muted">
              Some buff timing is unavailable.
            </p>
          </div>
        </div>
        <p v-if="!websocket.isConnected" class="mt-1 text-xs text-muted">
          Snapshot; live updates disconnected
        </p>
      </template>
    </div>
  </section>
</template>
