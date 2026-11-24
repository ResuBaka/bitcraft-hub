<script setup lang="ts">
import { formatFoodTimer, meterValue, summarizeFoodBuffs } from "~/utils/monitorBars";
import { overviewStatsKey, useOverviewStats } from "~/composables/useOverviewStats";
import { FoodExpiryTracker, vitalAlerts } from "~/utils/accountOverviewAlerts";
import { overviewActivityKey } from "~/composables/useOverviewActivity";

const props = defineProps<{ playerId: string; title?: string }>();
const websocket = useWebsocketStore();
const { data, receivedAt, pending, error, refreshStats } =
  inject(overviewStatsKey, null) ?? useOverviewStats(() => props.playerId);
const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | undefined;
const notify = useOverviewNotify();
const activity = inject(overviewActivityKey, null);
const foodExpiry = new FoodExpiryTracker();
const lastVitals = new Map<string, boolean>();
watch(
  data,
  (next) => {
    now.value = Date.now();
    if (!notify || !next) return;
    for (const event of vitalAlerts(next, activity?.current.value.recipeKind)) {
      if (lastVitals.get(event.type) !== event.active) {
        lastVitals.set(event.type, event.active);
        notify(props.playerId, event);
      }
    }
    for (const event of foodExpiry.snapshot(next, receivedAt.value)) notify(props.playerId, event);
  },
  { immediate: true },
);
onMounted(() => {
  timer = setInterval(() => {
    now.value = Date.now();
    for (const event of foodExpiry.tick(now.value)) notify?.(props.playerId, event);
  }, 1000);
});
onBeforeUnmount(() => {
  if (timer) clearInterval(timer);
});

const foodSummary = computed(() =>
  summarizeFoodBuffs(
    (data.value?.buffs ?? []).map((buff) => ({
      ...buff,
      remaining_seconds: buff.remaining_seconds == null ? null : Number(buff.remaining_seconds),
    })),
    Math.max(0, Math.floor((now.value - receivedAt.value) / 1000)),
  ),
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
