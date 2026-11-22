<script setup lang="ts">
import { watchDebounced } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { PlayerActivitySnapshot } from "~/types/PlayerActivitySnapshot";
import {
  ActivityLinger,
  activityCraftProgress,
  currentPlayerActivity,
} from "~/utils/playerActivity";
import { SnapshotRevision } from "~/utils/snapshotRevision";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";

const props = defineProps<{ playerId: string }>();
const websocket = useWebsocketStore();
const revisions = new SnapshotRevision();
const requestRevisions = new WeakMap<object, number>();
let responseRevision = revisions.capture();
const snapshot = shallowRef<PlayerActivitySnapshot | null>(null);
const clock = ref(Date.now());
const serverOffset = ref(0);
let timer: ReturnType<typeof setInterval> | undefined;

function applySnapshot(value: PlayerActivitySnapshot) {
  if (value.player_id !== props.playerId) return;
  const current = Date.now();
  const serverTime = Number(value.server_time_ms);
  serverOffset.value = Number.isFinite(serverTime) ? serverTime - current : 0;
  clock.value = current;
  snapshot.value = value;
}

const {
  data: fetched,
  pending,
  error,
  refresh,
} = useLazyFetchMsPack<PlayerActivitySnapshot>(
  () => `/api/bitcraft/players/${encodeURIComponent(props.playerId)}/activity`,
  {
    server: false,
    // A cached timestamp cannot be used to infer the current action on remount.
    getCachedData: () => undefined,
    onRequest: ({ options }) => {
      requestRevisions.set(options, revisions.capture());
    },
    onResponse: ({ options }) => {
      responseRevision = requestRevisions.get(options) ?? -1;
    },
  },
);
watch(
  fetched,
  (value) => {
    if (value && revisions.isCurrent(responseRevision)) applySnapshot(value);
  },
  { immediate: true },
);

const linger = new ActivityLinger();
const current = shallowRef(currentPlayerActivity(null, clock.value));
watch(
  [snapshot, clock, serverOffset],
  () => {
    current.value = linger.update(
      currentPlayerActivity(snapshot.value, clock.value + serverOffset.value),
      clock.value,
    );
  },
  { immediate: true, flush: "sync" },
);
const { data: crafts } = usePlayerCraftSnapshot(() => props.playerId, "Active");
const craftProgress = computed(() => activityCraftProgress(current.value, crafts.value));
onMounted(() => {
  timer = setInterval(() => {
    clock.value = Date.now();
  }, 500);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

let dirty = false;
let refreshing = false;
async function refreshActivity() {
  dirty = true;
  if (refreshing) return;
  refreshing = true;
  try {
    while (dirty) {
      dirty = false;
      if (pending.value) await refresh({ dedupe: "defer" });
      await refresh({ dedupe: "defer" });
    }
  } finally {
    refreshing = false;
  }
}
const invalidations = ref(0);
registerWebsocketMessageHandler(
  "PlayerActivityChanged",
  () => ["player_activity", `player_activity.${props.playerId}`],
  (message) => {
    if (message.player_id !== null && message.player_id !== props.playerId) return;
    revisions.invalidate();
    if (message.player_id === props.playerId && message.activity?.player_id === props.playerId)
      applySnapshot(message.activity);
    else invalidations.value++;
  },
);
registerWebsocketMessageHandler(
  "PlayerLocation",
  () => [`player_location.${props.playerId}`],
  (message) => {
    if (String(message.entity_id) !== props.playerId || !snapshot.value) return;
    snapshot.value = { ...snapshot.value, walking: message.location?.is_walking ?? null };
  },
);
let lastSignedIn: boolean | undefined;
registerWebsocketMessageHandler(
  "PlayerPresenceChanged",
  () => [`player_presence.${props.playerId}`],
  (player) => {
    if (player.player_id !== props.playerId || lastSignedIn === player.signed_in) return;
    lastSignedIn = player.signed_in;
    revisions.invalidate();
    if (snapshot.value) snapshot.value = { ...snapshot.value, signed_in: player.signed_in };
    invalidations.value++;
  },
);
watchDebounced(invalidations, () => void refreshActivity(), { debounce: 300, maxWait: 2000 });
let awaitingAck = true;
registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (awaitingAck && hasSubscribedTopic(topics, "player_activity", props.playerId)) {
    awaitingAck = false;
    revisions.invalidate();
    invalidations.value++;
  }
});
websocket.sendMessage({ t: "ListSubscribedTopics" });
watch(
  () => websocket.isConnected,
  (connected) => {
    awaitingAck = true;
    if (connected) invalidations.value++;
  },
);
function onVisibilityChange() {
  if (document.visibilityState === "visible") void refreshActivity();
}
onMounted(() => document.addEventListener("visibilitychange", onVisibilityChange));
onBeforeUnmount(() => document.removeEventListener("visibilitychange", onVisibilityChange));

type RecipeDetails = { kind: "craft" | "gather"; id: number; name: string | null };
const recipeKey = computed(() =>
  current.value.recipeKind && current.value.recipeId != null
    ? `${current.value.recipeKind}:${current.value.recipeId}`
    : null,
);
const recipeNames = ref<Record<string, string | null>>({});
const { data: recipe, refresh: refreshRecipe } = useLazyFetchMsPack<RecipeDetails>(
  () =>
    `/api/bitcraft/activity/recipe/${current.value.recipeKind ?? "craft"}/${current.value.recipeId ?? 0}`,
  { server: false, immediate: false, watch: false, key: `activity-recipe:${props.playerId}` },
);
watch(recipe, (value) => {
  if (value) recipeNames.value[`${value.kind}:${value.id}`] = value.name;
});
watch(
  recipeKey,
  (key) => {
    if (key && !(key in recipeNames.value)) void refreshRecipe();
  },
  { immediate: true },
);
const recipeName = computed(() => (recipeKey.value ? recipeNames.value[recipeKey.value] : null));
</script>

<template>
  <section
    class="@container flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default p-1.5"
  >
    <div class="flex shrink-0 items-center justify-between gap-1 overflow-x-auto">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <h3 class="truncate text-base font-semibold">Activity</h3>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <UButton
          icon="i-lucide-refresh-cw"
          size="xs"
          color="neutral"
          variant="ghost"
          :loading="pending"
          aria-label="Refresh activity"
          @click="refreshActivity"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <p v-if="error && !snapshot" class="mt-1 text-sm text-error">
        Could not load activity. Refresh to retry.
      </p>
      <div v-else class="flex items-start gap-1.5 text-sm">
        <UIcon :name="current.icon" class="mt-0.5 size-4 shrink-0 text-muted" />
        <div class="min-w-0 flex-1">
          <p class="break-words leading-tight">
            <span class="font-medium">{{ current.label }}</span>
            <span v-if="current.recipeKind" class="text-muted">
              ·
              {{
                recipeName ??
                (current.recipeId == null ? "Recipe unavailable" : `Recipe #${current.recipeId}`)
              }}
            </span>
          </p>
          <template v-if="current.recipeKind === 'craft'">
            <template v-if="craftProgress">
              <div class="mt-0.5 flex items-center gap-1.5">
                <progress
                  class="block h-1.5 min-w-0 flex-1 accent-primary"
                  :value="craftProgress.percent"
                  max="100"
                  aria-label="Craft progress"
                />
                <span class="shrink-0 text-xs text-muted"
                  >{{ craftProgress.progress }} / {{ craftProgress.total }}</span
                >
              </div>
            </template>
            <p v-else class="text-xs text-muted">Progress unavailable</p>
          </template>
          <p v-else-if="current.recipeKind === 'gather'" class="text-xs text-muted">
            Resource health unavailable
          </p>
        </div>
      </div>
      <p v-if="snapshot && !websocket.isConnected" class="mt-1 text-xs text-muted">
        Snapshot; live updates disconnected
      </p>
    </div>
  </section>
</template>
