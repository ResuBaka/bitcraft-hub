<script setup lang="ts">
import { passiveSummaryTiming, summarizePassiveCrafts } from "~/utils/passiveCraftSummary";
import { getTierColor } from "~/utils";
import { passiveReadyAlerts } from "~/utils/accountOverviewAlerts";

const props = defineProps<{ playerId: string }>();
const selected = defineModel<string[]>("claims", { required: true });
const { data, pending, error, refresh, liveConnected } = usePlayerCraftSnapshot(
  () => props.playerId,
  "Passive",
);
const passiveCrafts = computed(() =>
  (data.value ?? []).filter((craft) => craft.kind === "Passive"),
);
const { options, filtered: crafts } = useCraftClaimFilter(selected, passiveCrafts);
const notify = useOverviewNotify();
watch(data, (next, previous) => {
  for (const event of passiveReadyAlerts(previous, next, selected.value))
    notify?.(props.playerId, event);
});

const summaries = computed(() => summarizePassiveCrafts(crafts.value));
const { seen, acknowledge } = useCraftAcknowledgements(props.playerId, "Passive", data);
const unseenSummaries = computed(
  () =>
    new Set(
      summaries.value
        .filter((summary) => summary.readyIds.some((id) => !seen.value.has(id)))
        .map((summary) => summary.key),
    ),
);
const now = ref(Date.now());
let clock: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  clock = setInterval(() => {
    now.value = Date.now();
  }, 1000);
});
onBeforeUnmount(() => {
  if (clock) clearInterval(clock);
});
</script>

<template>
  <section
    class="@container flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default p-1.5"
  >
    <div class="flex shrink-0 items-center justify-between gap-1 overflow-x-auto">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <h3 class="truncate text-base font-semibold" title="Passive projects owned by this player">
          Passive Crafts
        </h3>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <span
          :title="liveConnected ? 'WebSocket connected' : 'WebSocket disconnected; use refresh'"
          :aria-label="
            liveConnected ? 'WebSocket connected' : 'WebSocket disconnected; use refresh'
          "
          role="status"
          :class="liveConnected ? 'bg-success' : 'bg-muted'"
          class="size-2 rounded-full"
        />
        <AccountOverviewCraftBlockSettings
          v-model="selected"
          :options="options"
          block-name="Passive crafts"
        />
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :loading="pending"
          aria-label="Refresh passive crafts"
          @click="refresh()"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <p v-if="error" class="mt-1 text-sm text-error">Could not load crafts. Refresh to retry.</p>
      <p v-if="pending && !data" class="mt-1 text-sm text-muted">Loading…</p>
      <template v-else-if="data">
        <p v-if="!crafts.length" class="mt-1 text-sm text-muted">
          {{ selected.length ? "No crafts match the selected claims" : "No passive crafts" }}
        </p>
        <ul v-else class="grid grid-cols-1 gap-x-2 gap-y-1 @lg:grid-cols-2 @4xl:grid-cols-3">
          <li
            v-for="summary in summaries"
            :key="summary.key"
            class="flex min-w-0 items-center gap-1.5 text-sm"
          >
            <BitcraftInventoryImg
              v-if="summary.item"
              :item="summary.item"
              :width="20"
              :height="20"
              class="shrink-0"
            />
            <div class="min-w-0 flex-1">
              <div class="flex items-baseline gap-x-1">
                <NuxtLink
                  v-if="summary.buildingId"
                  :to="`/buildings/${summary.buildingId}`"
                  class="min-w-0 flex-1 truncate font-medium hover:underline"
                  :class="summary.item?.tier != null ? getTierColor(summary.item.tier) : undefined"
                  :title="summary.name"
                >
                  {{ summary.name }}
                </NuxtLink>
                <span
                  v-else
                  class="min-w-0 flex-1 truncate font-medium"
                  :class="summary.item?.tier != null ? getTierColor(summary.item.tier) : undefined"
                  :title="summary.name"
                  >{{ summary.name }}</span
                >
                <span
                  class="shrink-0 text-xs text-muted"
                  :title="passiveSummaryTiming(summary, now).detail"
                  :aria-label="passiveSummaryTiming(summary, now).detail"
                >
                  {{ passiveSummaryTiming(summary, now).label }}
                </span>
                <span
                  class="shrink-0 text-xs text-muted"
                  :aria-label="`${summary.ready} of ${summary.total} projects ready to collect`"
                  >{{ summary.ready }}/{{ summary.total }} ready</span
                >
              </div>
              <AccountOverviewCraftCompletionBar
                v-if="unseenSummaries.has(summary.key)"
                :value="summary.ready"
                :max="summary.total"
                :description="`Mark ready ${summary.name} projects as seen (${summary.ready} of ${summary.total} ready)`"
                @acknowledge="acknowledge(summary.readyIds)"
              />
              <progress
                v-else
                :value="summary.ready"
                :max="summary.total"
                :aria-label="`${summary.name} projects ready to collect`"
                :title="`${summary.ready} of ${summary.total} projects ready to collect`"
                class="block h-1.5 w-full accent-primary"
              />
            </div>
          </li>
        </ul>
      </template>
    </div>
  </section>
</template>
