import type { InjectionKey } from "vue";
import { useIntervalFn, watchDebounced } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { PlayerActivitySnapshot } from "~/types/PlayerActivitySnapshot";
import { ActivityLinger, currentPlayerActivity } from "~/utils/playerActivity";
import { SnapshotRevision } from "~/utils/snapshotRevision";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";

// Owned by the player dashboard so stats can see the same activity even when
// the Activity block is hidden, without another fetch or socket subscription.
export const overviewActivityKey: InjectionKey<ReturnType<typeof useOverviewActivity>> = Symbol(
  "overview-player-activity",
);

export function useOverviewActivity(playerId: string) {
  const websocket = useWebsocketStore();
  const revisions = new SnapshotRevision();
  const requestRevisions = new WeakMap<object, number>();
  let responseRevision = revisions.capture();
  const snapshot = shallowRef<PlayerActivitySnapshot | null>(null);
  const signedIn = ref<boolean | null>(null);
  const clock = ref(Date.now());
  const serverOffset = ref(0);
  function applySnapshot(value: PlayerActivitySnapshot) {
    if (value.player_id !== playerId) return;
    clock.value = Date.now();
    const serverTime = Number(value.server_time_ms);
    serverOffset.value = Number.isFinite(serverTime) ? serverTime - clock.value : 0;
    signedIn.value = value.signed_in;
    snapshot.value = value;
  }
  const {
    data: fetched,
    pending,
    error,
    refresh,
  } = useLazyFetchMsPack<PlayerActivitySnapshot>(
    () => `/api/bitcraft/players/${encodeURIComponent(playerId)}/activity`,
    {
      server: false,
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

  const raw = computed(() =>
    currentPlayerActivity(snapshot.value, clock.value + serverOffset.value),
  );
  const linger = new ActivityLinger();
  const current = shallowRef(raw.value);
  watch(
    raw,
    (value) => {
      current.value = linger.update(value, clock.value);
    },
    { immediate: true },
  );
  useIntervalFn(() => {
    clock.value = Date.now();
  }, 500);

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
    ["player_activity", `player_activity.${playerId}`],
    (message) => {
      if (message.player_id !== null && message.player_id !== playerId) return;
      revisions.invalidate();
      if (message.player_id === playerId && message.activity?.player_id === playerId)
        applySnapshot(message.activity);
      else invalidations.value++;
    },
  );
  registerWebsocketMessageHandler("PlayerLocation", [`player_location.${playerId}`], (message) => {
    if (String(message.entity_id) === playerId && snapshot.value)
      snapshot.value = { ...snapshot.value, walking: message.location?.is_walking ?? null };
  });
  registerWebsocketMessageHandler(
    "PlayerPresenceChanged",
    [`player_presence.${playerId}`],
    (player) => {
      if (player.player_id !== playerId || signedIn.value === player.signed_in) return;
      signedIn.value = player.signed_in;
      revisions.invalidate();
      if (snapshot.value) snapshot.value = { ...snapshot.value, signed_in: player.signed_in };
      invalidations.value++;
    },
  );
  watchDebounced(invalidations, () => void refreshActivity(), { debounce: 300, maxWait: 2000 });
  let awaitingAck = true;
  registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
    if (awaitingAck && hasSubscribedTopic(topics, "player_activity", playerId)) {
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
  return { snapshot, signedIn, clock, raw, current, pending, error, refreshActivity };
}
