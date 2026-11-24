<script setup lang="ts">
import { useDocumentVisibility, useIntervalFn, useTimestamp } from "@vueuse/core";
import { getTierColor } from "~/utils";
import { formatCountdown } from "~/utils/passiveCraftSummary";

type Placeable = {
  id: string;
  owner_id: string;
  name: string;
  tier: number | null;
  growing: boolean;
  ends_at_ms: number | bigint | null;
  interactions: string[];
};

const props = defineProps<{ playerId: string; compact?: boolean }>();
const { data, pending, error, refresh } = useLazyFetchMsPack<Placeable[]>(
  () => `/api/bitcraft/players/${encodeURIComponent(props.playerId)}/placeables`,
  { server: false },
);
// Nuxt can retain the previous player's snapshot while switching keys.
const rows = computed(() =>
  data.value?.every((row) => row.owner_id === props.playerId) ? data.value : undefined,
);
const visible = useDocumentVisibility();
const now = useTimestamp({ interval: 1000 });
const reload = () => refresh({ dedupe: "defer" });
useIntervalFn(() => {
  if (visible.value === "visible") void reload();
}, 15000);
watch(visible, (value) => {
  if (value === "visible") void reload();
});
function stage(row: Placeable) {
  if (!row.growing) return "Placed";
  if (row.ends_at_ms === null) return "Growing";
  const left = Number(row.ends_at_ms) - now.value;
  return left > 0 ? `Next stage in ${formatCountdown(left)}` : "Awaiting next stage";
}
</script>

<template>
  <section
    class="flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default"
    :class="compact ? 'p-1.5' : 'gap-3 p-4'"
  >
    <div class="flex shrink-0 items-center justify-between gap-1">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <component
          :is="compact ? 'h3' : 'h2'"
          class="truncate font-semibold"
          :class="compact ? 'text-base' : 'text-lg'"
        >
          Placeables
        </component>
      </div>
      <div class="flex items-center gap-1">
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          aria-label="Refresh placeables"
          :loading="pending"
          @click="reload()"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto text-sm">
      <p v-if="error" class="text-error">Could not load placeables. Refresh to retry.</p>
      <p v-else-if="!rows" class="text-muted">Loading placeables…</p>
      <p v-else-if="!rows.length" class="text-muted">No owned placeables</p>
      <ul
        v-else
        class="grid grid-cols-[repeat(auto-fill,minmax(min(100%,15rem),1fr))] content-start gap-x-3 gap-y-1"
      >
        <li
          v-for="row in rows"
          :key="row.id"
          class="min-w-0 border-t border-default py-1 leading-tight"
        >
          <span
            class="font-medium wrap-break-word"
            :class="row.tier != null && row.tier > 0 ? getTierColor(row.tier) : undefined"
            >{{ row.name }}</span
          >
          <p class="flex flex-wrap items-baseline gap-x-2 text-xs text-muted">
            <span>{{ stage(row) }}</span>
            <span v-if="row.interactions.length" class="wrap-break-word">
              <span aria-hidden="true">· </span><span class="sr-only">Interactions: </span>
              {{ row.interactions.join(", ") }}
            </span>
          </p>
        </li>
      </ul>
    </div>
  </section>
</template>
