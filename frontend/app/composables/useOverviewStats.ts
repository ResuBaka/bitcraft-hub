import type { InjectionKey } from "vue";
import { watchDebounced } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import type { StatsSnapshot } from "~/types/StatsSnapshot";
import { SnapshotRevision } from "~/utils/snapshotRevision";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";
import { statsMessageAction } from "~/utils/statsMessage";

export const overviewStatsKey: InjectionKey<ReturnType<typeof useOverviewStats>> =
  Symbol("overview-player-stats");

export function useOverviewStats(playerId: () => string) {
  const revisions = new SnapshotRevision();
  let responseRevision = revisions.capture();
  const requestRevisions = new WeakMap<object, number>();
  const data = shallowRef<StatsSnapshot | null>(null);
  const receivedAt = ref(Date.now());
  function apply(snapshot: StatsSnapshot) {
    receivedAt.value = Date.now();
    data.value = snapshot;
  }
  const {
    data: fetched,
    pending,
    error,
    refresh,
  } = useLazyFetchMsPack<StatsSnapshot>(
    () => `/api/bitcraft/players/${encodeURIComponent(playerId())}/stats`,
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
  watch(
    fetched,
    (snapshot) => {
      if (snapshot?.player_id === playerId() && revisions.isCurrent(responseRevision))
        apply(snapshot);
    },
    { immediate: true },
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
        if (pending.value) await refresh({ dedupe: "defer" });
        await refresh({ dedupe: "defer" });
      }
    } finally {
      refreshing = false;
    }
  }
  registerWebsocketMessageHandler(
    "PlayerStatsChanged",
    () => ["player_stats", `player_stats.${playerId()}`],
    (message) => {
      const action = statsMessageAction(playerId(), message);
      if (action.type === "ignore") return;
      revisions.invalidate();
      if (action.type === "snapshot") apply(action.snapshot);
      else changes.value++;
    },
  );
  let lastSignedIn: boolean | undefined;
  registerWebsocketMessageHandler(
    "PlayerPresenceChanged",
    () => [`player_presence.${playerId()}`],
    (player) => {
      if (player.player_id !== playerId() || lastSignedIn === player.signed_in) return;
      lastSignedIn = player.signed_in;
      revisions.invalidate();
      changes.value++;
    },
  );
  watchDebounced(changes, () => void refreshStats(), { debounce: 400, maxWait: 2000 });
  const websocket = useWebsocketStore();
  let awaitingAck = true;
  registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
    if (awaitingAck && hasSubscribedTopic(topics, "player_stats", playerId())) {
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
  return { data, receivedAt, pending, error, refreshStats };
}
