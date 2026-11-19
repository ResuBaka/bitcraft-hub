import { watchDebounced } from "@vueuse/core";
import type { MaybeRefOrGetter } from "vue";
import type { PlayerCraft } from "~/types/PlayerCraft";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import { applyCraftProgress, applyPassiveCraftUpdate } from "~/utils/craftProgress";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";

export function usePlayerCraftSnapshot(
  playerId: MaybeRefOrGetter<string>,
  kind: "Active" | "Passive",
) {
  // Each block owns its snapshot and reconciliation, so removing or refreshing one
  // cannot cancel the other block's in-flight request.
  const { data, pending, error, refresh } = useLazyFetchMsPack<PlayerCraft[]>(
    () => `/api/bitcraft/players/${encodeURIComponent(toValue(playerId))}/crafting`,
    { server: false, key: computed(() => `monitor-crafting:${kind}:${toValue(playerId)}`) },
  );
  const websocket = useWebsocketStore();
  const liveVersion = ref(0);
  let awaitingAck = true;
  let refreshing = false;
  let dirty = false;

  async function refreshAll() {
    dirty = true;
    if (refreshing) return;
    refreshing = true;
    try {
      while (dirty) {
        dirty = false;
        await refresh();
      }
    } finally {
      refreshing = false;
    }
  }

  registerWebsocketMessageHandler(
    "PlayerCraftingChanged",
    () => ["player_crafting", `player_crafting.${toValue(playerId)}`],
    (message) => {
      if (message.player_id === null || message.player_id === toValue(playerId))
        liveVersion.value++;
    },
  );

  if (kind === "Active") {
    registerWebsocketMessageHandler(
      "CraftProgressChanged",
      () => [`player_craft_progress.${toValue(playerId)}`],
      (message) => {
        if (message.player_id !== toValue(playerId)) return;
        const updated = data.value && applyCraftProgress(data.value, message);
        if (updated) data.value = updated;
        if (!updated || pending.value) liveVersion.value++;
      },
    );
  } else {
    registerWebsocketMessageHandler(
      "PassiveCraftChanged",
      () => [`player_passive_craft.${toValue(playerId)}`],
      (message) => {
        if (message.player_id !== toValue(playerId)) return;
        const updated = data.value && applyPassiveCraftUpdate(data.value, message);
        if (updated) data.value = updated;
        if (!updated || pending.value) liveVersion.value++;
      },
    );
  }

  // Reconcile once the server acknowledges the subscription, closing the gap
  // between the initial HTTP snapshot and the subscription becoming active.
  registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
    const id = toValue(playerId);
    const detailTopic = kind === "Active" ? "player_craft_progress" : "player_passive_craft";
    if (
      awaitingAck &&
      hasSubscribedTopic(topics, "player_crafting", id) &&
      hasSubscribedTopic(topics, detailTopic, id)
    ) {
      awaitingAck = false;
      liveVersion.value++;
    }
  });
  websocket.sendMessage({ t: "ListSubscribedTopics" });
  watchDebounced(liveVersion, () => void refreshAll(), { debounce: 250, maxWait: 2000 });
  watch(
    () => websocket.isConnected,
    (connected) => {
      awaitingAck = true;
      if (connected) liveVersion.value++;
    },
  );
  watch(
    () => toValue(playerId),
    () => {
      awaitingAck = true;
      websocket.sendMessage({ t: "ListSubscribedTopics" });
    },
  );
  function onVisibilityChange() {
    if (document.visibilityState === "visible") void refreshAll();
  }
  onMounted(() => document.addEventListener("visibilitychange", onVisibilityChange));
  onBeforeUnmount(() => document.removeEventListener("visibilitychange", onVisibilityChange));

  // Active locks can expire without another database change.
  let expiryTimer: ReturnType<typeof setTimeout> | undefined;
  if (kind === "Active") {
    watch(
      data,
      (crafts) => {
        if (expiryTimer) clearTimeout(expiryTimer);
        const now = Date.now();
        const next = (crafts ?? [])
          .filter(
            (craft) => craft.kind === "Active" && craft.status === "Active" && craft.timestamp,
          )
          .map((craft) => Date.parse(craft.timestamp!))
          .filter((time) => Number.isFinite(time) && time > now)
          .reduce((earliest, time) => Math.min(earliest, time), Infinity);
        if (Number.isFinite(next)) {
          expiryTimer = setTimeout(
            () => void refreshAll(),
            Math.min(next - now + 250, 2_147_483_647),
          );
        }
      },
      { immediate: true },
    );
    onBeforeUnmount(() => {
      if (expiryTimer) clearTimeout(expiryTimer);
    });
  }

  return {
    data,
    pending,
    error,
    refresh: refreshAll,
    liveConnected: computed(() => websocket.isConnected),
  };
}
