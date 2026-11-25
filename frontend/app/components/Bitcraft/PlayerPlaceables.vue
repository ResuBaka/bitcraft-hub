<script setup lang="ts">
import { useDocumentVisibility, useIntervalFn, useTimestamp } from "@vueuse/core";
import { getTierColor } from "~/utils";
import { formatCountdown } from "~/utils/countdown";
import { formatNumber } from "~/utils/formatNumber";

type Stage = {
  owner_id: string;
  outcome_id: number;
  outcome_name: string;
  outcome_tier: number | null;
  stage_id: number;
  name: string;
  ready: boolean;
  growing: boolean;
  ends_at_ms: number | bigint | null;
  interactions: string[];
  quantity: number | bigint;
};

const props = defineProps<{ playerId: string; compact?: boolean; farming?: boolean }>();
const noun = computed(() => (props.farming ? "crops" : "placeables"));
const { data, pending, error, refresh } = useLazyFetchMsPack<Stage[]>(
  () =>
    `/api/bitcraft/players/${encodeURIComponent(props.playerId)}/placeable-summary?farming=${!!props.farming}`,
  { server: false },
);
// Nuxt can retain the previous player's snapshot while switching keys.
const rows = computed(() =>
  data.value?.every((row) => row.owner_id === props.playerId) ? data.value : undefined,
);
const groups = computed(() => {
  const result = new Map<
    number,
    { name: string; tier: number | null; total: number; ready: number; stages: Stage[] }
  >();
  for (const row of rows.value ?? []) {
    let group = result.get(row.outcome_id);
    if (!group) {
      group = { name: row.outcome_name, tier: row.outcome_tier, total: 0, ready: 0, stages: [] };
      result.set(row.outcome_id, group);
    }
    group.stages.push(row);
    group.total += Number(row.quantity);
    if (row.ready) group.ready += Number(row.quantity);
  }
  return result;
});
const visible = useDocumentVisibility();
const now = useTimestamp({ interval: 1000 });
const reload = () => refresh({ dedupe: "defer" });
useIntervalFn(() => {
  if (visible.value === "visible") void reload();
}, 15000);
watch(visible, (value) => {
  if (value === "visible") void reload();
});
function stage(row: Stage) {
  if (row.ends_at_ms === null) return "Growing";
  const left = Number(row.ends_at_ms) - now.value;
  const next = Number(row.quantity) > 1 ? "First change" : "Next stage";
  return left > 0 ? `${next} in ${formatCountdown(left)}` : "Awaiting next stage";
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
          {{ farming ? "Farming" : "Placeables" }}
        </component>
      </div>
      <div class="flex items-center gap-1">
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :aria-label="`Refresh ${noun}`"
          :loading="pending"
          @click="reload()"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto text-sm">
      <p v-if="error" class="text-error">Could not load {{ noun }}. Refresh to retry.</p>
      <p v-else-if="!rows" class="text-muted">Loading {{ noun }}…</p>
      <p v-else-if="!rows.length" class="text-muted">No owned {{ noun }}</p>
      <ul
        v-else
        class="grid grid-cols-[repeat(auto-fill,minmax(min(100%,15rem),1fr))] content-start gap-x-3 gap-y-1"
      >
        <li
          v-for="[id, group] in groups"
          :key="id"
          class="min-w-0 border-t border-default py-1 leading-tight"
        >
          <span
            class="font-medium wrap-break-word"
            :class="group.tier != null && group.tier > 0 ? getTierColor(group.tier) : undefined"
            >{{ group.name }}</span
          >
          <span
            class="ms-1 whitespace-nowrap text-xs tabular-nums"
            :class="group.ready ? 'text-success' : 'text-muted'"
            :title="
              farming
                ? 'Ready to harvest'
                : 'Ready to collect the outcome’s reward'
            "
             >{{ formatNumber(group.ready) }}/{{ formatNumber(group.total) }} ready</span
          >
          <ul class="text-xs text-muted">
            <li
              v-for="row in group.stages"
              :key="`${row.stage_id}:${row.growing}`"
              class="wrap-break-word"
            >
              <span class="font-medium">{{ formatNumber(row.quantity) }} × {{ row.name }}</span>
              <span v-if="row.interactions.length"> · {{ row.interactions.join(", ") }}</span>
              <span v-if="row.growing"> · {{ stage(row) }}</span>
            </li>
          </ul>
        </li>
      </ul>
    </div>
  </section>
</template>
