<script setup lang="ts">
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import { hasSubscribedTopic } from "~/utils/websocketTopicRegistry";
import { formatNumber } from "~/utils/formatNumber";

// The Activity block keys this component by node; changing targets releases
// the previous subscription and never carries its health to the next node.
const props = defineProps<{ targetId: string; maxHealth?: number | null }>();
const websocket = useWebsocketStore();
const baseURL = useRuntimeConfig().public.api.base;
const health = ref<number | null>(null);
const status = ref<"loading" | "waiting" | "error" | "removed">("loading");
let revision = 0;
let request: AbortController | undefined;
let retry: ReturnType<typeof setTimeout> | undefined;
let retryMs = 1500;

function cancelRetry() {
  clearTimeout(retry);
  retry = undefined;
}

// A new node may be visible in activity before its HP is committed to
// Postgres. Some nodes don't change HP, so waiting for a hit isn't enough.
function retryLookup() {
  if (document.visibilityState !== "visible") return;
  retry = setTimeout(() => void refresh(), retryMs);
  retryMs = Math.min(retryMs * 2, 15000);
}

async function refresh() {
  cancelRetry();
  request?.abort();
  request = new AbortController();
  const snapshot = ++revision;
  try {
    const data = await $fetch<{ health: number | null }>(
      `/api/bitcraft/resources/${encodeURIComponent(props.targetId)}/health`,
      {
        baseURL,
        headers: { Accept: "application/json" },
        signal: request.signal,
        timeout: 5000,
        retry: 0,
      },
    );
    if (snapshot !== revision) return;
    health.value = data.health;
    if (data.health == null) {
      status.value = "waiting";
      retryLookup();
    } else {
      retryMs = 1500;
    }
  } catch {
    if (snapshot !== revision) return;
    health.value = null;
    status.value = "error";
    retryLookup();
  }
}
defineExpose({ refresh });

registerWebsocketMessageHandler(
  "ResourceHealthChanged",
  ["resource_health", `resource_health.${props.targetId}`],
  (message) => {
    if (message.target_id === null) void refresh(); // Regional snapshot reconciled.
    else if (message.target_id === props.targetId) {
      revision++;
      cancelRetry();
      retryMs = 1500;
      health.value = message.health;
      if (message.health === null) status.value = "removed";
    }
  },
);
let awaitingAck = true;
watch(
  () => websocket.isConnected,
  () => {
    awaitingAck = true;
  },
);
registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (awaitingAck && hasSubscribedTopic(topics, "resource_health", props.targetId)) {
    awaitingAck = false;
    void refresh();
  }
});
function onVisibilityChange() {
  if (document.visibilityState !== "visible") cancelRetry();
  else if (health.value === null && status.value !== "removed") void refresh();
}
onMounted(() => {
  document.addEventListener("visibilitychange", onVisibilityChange);
  void refresh();
  websocket.sendMessage({ t: "ListSubscribedTopics" });
});
onBeforeUnmount(() => {
  document.removeEventListener("visibilitychange", onVisibilityChange);
  cancelRetry();
  revision++;
  request?.abort();
});
</script>

<template>
  <div v-if="health !== null" class="mt-0.5 flex items-center gap-1.5">
    <progress
      v-if="maxHealth != null && maxHealth > 0"
      :value="health"
      :max="maxHealth"
      class="block h-1.5 min-w-0 flex-1 accent-primary"
      aria-label="Resource health"
    />
    <span class="shrink-0 text-xs text-muted">
      {{ formatNumber(health)
      }}<template v-if="maxHealth != null && maxHealth > 0"> / {{ formatNumber(maxHealth) }}</template> HP
    </span>
  </div>
  <p v-else class="text-xs text-muted">
    {{
      status === "removed"
        ? "Resource no longer present"
        : status === "waiting"
          ? "Waiting for resource health…"
          : status === "error"
            ? "Could not load resource health. Retrying…"
            : "Loading resource health…"
    }}
  </p>
</template>
