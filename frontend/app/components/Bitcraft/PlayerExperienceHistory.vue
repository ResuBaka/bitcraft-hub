<script setup lang="ts">
import { watchDebounced } from "@vueuse/shared";
import { registerWebsocketMessageHandler } from "~/composables/websocket";

const props = defineProps<{ playerId?: string; claimId?: string }>();
type Point = {
  day: string;
  experience_rate: number;
  bucket_seconds: number;
  sample_interval_seconds?: number;
  skill_id?: number;
};
type Skill = { id: number; name: string };
type Scale = "hour" | "day" | "week" | "month";

const minute = 60_000;
const hour = 60 * minute;
const day = 24 * hour;
// Match the API's point spacing to fill periods where no XP was recorded.
const buckets = {
  hour: { interval: minute, label: "minute", unit: "1m" },
  day: { interval: 30 * minute, label: "10 minutes", unit: "10m" },
  week: { interval: 3 * hour, label: "hour", unit: "1h" },
  month: { interval: 7 * day, label: "day", unit: "1d" },
} satisfies Record<Scale, { interval: number; label: string; unit: string }>;
const scale = ref<Scale>("hour");
const selectedBucket = computed(() => buckets[scale.value]);
const locationFilter = ref("all");
const selectedSkills = ref<string[]>(["all"]);
const { data: skills } = await useLazyFetchMsPack<Skill[]>(
  () => "/api/bitcraft/experience/history/skills",
);
// Match the skill icons used by the leaderboards.
const skillIcons: Record<string, string> = {
  Fishing: "i-mdi-fish",
  Mining: "i-mdi-pickaxe",
  Woodcutting: "i-mdi-forest",
  Farming: "i-mdi-sprout",
  Carpentry: "i-mdi-hand-saw",
  Foraging: "i-mdi-leaf",
  Forestry: "i-mdi-axe",
  Masonry: "i-mdi-screwdriver",
  Smithing: "i-mdi-anvil",
  Scholar: "i-mdi-school",
  Hunting: "i-mdi-bow-arrow",
  Cooking: "i-mdi-stove",
  Leatherworking: "i-mdi-bag-personal",
  Tailoring: "i-mdi-tshirt-crew",
  Sailing: "i-mdi-sail-boat",
  Slayer: "i-mdi-sword",
  Taming: "i-mdi-paw",
  Construction: "i-mdi-hard-hat",
};
const skillOptions = computed(() => [
  { label: "All skills", value: "all", icon: "i-mdi-chart-line" },
  { label: "Total XP", value: "total", icon: "i-mdi-star-circle" },
  ...(skills.value ?? []).map((skill) => ({
    label: skill.name,
    value: String(skill.id),
    icon: skillIcons[skill.name] ?? "i-mdi-star-outline",
  })),
]);
const { data, pending, error, refresh } = await useLazyFetchMsPack<Point[]>(() => {
  const query = new URLSearchParams({ scale: scale.value, group_by_skill: "true" });
  if (!props.claimId && locationFilter.value !== "all") query.set("location", locationFilter.value);
  return props.claimId
    ? `/api/bitcraft/claims/${props.claimId}/experience/history?${query}`
    : `/api/bitcraft/experience/${props.playerId}/history?${query}`;
});
// Older API versions use five-minute buckets for the hour view and omit sample spacing.
const legacyHour = computed(() => scale.value === "hour" && data.value?.some(
  (point) => point.sample_interval_seconds == null && point.bucket_seconds === 5 * minute / 1000,
));
const rollingHour = computed(() => scale.value === "hour" && !legacyHour.value);

const historyVersion = ref(0);
if (props.claimId) {
  registerWebsocketMessageHandler("ClaimExperienceHistoryChanged", () => [`claim_experience_history.${props.claimId}`],
    (message) => { if (String(message.claim_id) === props.claimId) historyVersion.value++; });
} else {
  registerWebsocketMessageHandler("ExperienceHistoryChanged", () => [`experience_history.${props.playerId}`],
    (message) => { if (String(message.user_id) === props.playerId) historyVersion.value++; });
}
watchDebounced(historyVersion, () => {
  if (document.visibilityState === "visible") refresh();
}, { debounce: 500, maxWait: 2000 });

const updateSelectedSkills = (next: string[]) => {
  const added = next.filter((value) => !selectedSkills.value.includes(value));
  if (added.includes("all")) {
    next = next.filter((value) => value === "all" || value === "total");
  } else if (added.some((value) => value !== "total")) {
    next = next.filter((value) => value !== "all");
  }
  selectedSkills.value = next;
};

let refreshTimer: ReturnType<typeof setInterval> | undefined;
const refreshWhenVisible = () => {
  if (document.visibilityState === "visible") refresh();
};
onMounted(() => {
  refreshTimer = setInterval(refreshWhenVisible, 30_000);
  document.addEventListener("visibilitychange", refreshWhenVisible);
});
onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer);
  document.removeEventListener("visibilitychange", refreshWhenVisible);
});

const format = new Intl.NumberFormat(undefined, { maximumFractionDigits: 2 });
const dateFormat = new Intl.DateTimeFormat(undefined, {
  month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZone: "UTC",
});
const bars = computed(() => {
  const interval = legacyHour.value ? 5 * minute : selectedBucket.value.interval;
  const duration = scale.value === "hour" ? hour : (scale.value === "day" ? 1 : scale.value === "week" ? 7 : 180) * day;
  // Weekly TimescaleDB buckets start on Monday, 1970-01-05 UTC.
  const origin = scale.value === "month" ? 4 * day : 0;
  const bucket = (time: number) => Math.floor((time - origin) / interval) * interval + origin;
  let latestSample: number | undefined;
  const rates = new Map<string, number>();
  for (const point of data.value ?? []) {
    if ((point.sample_interval_seconds ?? point.bucket_seconds) !== interval / 1000) continue;
    const time = Date.parse(point.day);
    latestSample = Math.max(latestSample ?? time, time);
    rates.set(`${time}:${point.skill_id}`, point.experience_rate);
    rates.set(`${time}:total`, (rates.get(`${time}:total`) ?? 0) + point.experience_rate);
  }
  const now = (rollingHour.value ? latestSample : undefined) ?? Date.now();
  const end = bucket(now);
  const result: { time: number; [key: string]: number }[] = [];
  for (let time = bucket(now - duration); time <= end; time += interval) {
    const row: { time: number; [key: string]: number } = { time };
    for (const skill of chartSkills.value) row[`skill_${skill.id}`] = rates.get(`${time}:${skill.id}`) ?? 0;
    row.total = rates.get(`${time}:total`) ?? 0;
    result.push(row);
  }
  return result;
});
const chartSkills = computed(() => {
  const result = new Map((skills.value ?? []).map((skill) => [skill.id, skill]));
  for (const point of data.value ?? []) {
    if (point.skill_id != null && !result.has(point.skill_id)) {
      result.set(point.skill_id, { id: point.skill_id, name: `Skill ${point.skill_id}` });
    }
  }
  return [...result.values()].sort((a, b) => a.name.localeCompare(b.name));
});
const chartCategories = computed(() => [
  ...(selectedSkills.value.includes("total") ? [{ key: "total", label: "Total XP" }] : []),
  ...chartSkills.value
    .filter((skill) => selectedSkills.value.includes("all") || selectedSkills.value.includes(String(skill.id)))
    .map((skill) => ({ key: `skill_${skill.id}`, label: skill.name })),
]);
const chartData = computed(() => bars.value.map((bar) => ({
  ...bar,
  period: `${dateFormat.format(bar.time)} UTC${rollingHour.value ? " · 5-minute rolling average" : ""}`,
})));
</script>

<template>
  <UCard :ui="{ body: 'p-4' }">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 class="text-lg font-semibold">Experience history</h2>
        <p class="text-xs text-gray-500">UTC · Average XP rate since tracking began</p>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <USelect v-if="!claimId" v-model="locationFilter" aria-label="XP location" :items="[
          { label: 'All locations', value: 'all' },
          { label: 'On a claim', value: 'claim' },
          { label: 'Unclaimed land', value: 'unclaimed' },
          { label: 'Location unknown', value: 'unknown' },
        ]" />
        <USelect
          :model-value="selectedSkills"
          :items="skillOptions"
          :icon="selectedSkills.length === 1 ? skillOptions.find((option) => option.value === selectedSkills[0])?.icon : undefined"
          multiple
          placeholder="Select skills"
          aria-label="Skills"
          @update:model-value="updateSelectedSkills"
        >
          <span v-if="selectedSkills.length === 1">
            {{ skillOptions.find((option) => option.value === selectedSkills[0])?.label }}
          </span>
          <span v-else-if="selectedSkills.length">{{ selectedSkills.length }} selected</span>
          <span v-else>Select skills</span>
        </USelect>
        <UButton v-for="period in (['hour', 'day', 'week', 'month'] as const)" :key="period"
          size="sm" color="neutral" :variant="scale === period ? 'solid' : 'soft'"
          @click="scale = period">{{ period === 'month' ? '6 months' : period }}</UButton>
      </div>
    </div>
    <p v-if="pending && !data" class="mt-4 text-sm text-gray-500">Loading experience history…</p>
    <p v-else-if="error && !data" class="mt-4 text-sm text-red-500">Could not load experience history.</p>
    <template v-else>
      <UChart
        v-if="chartCategories.length"
        class="mt-4"
        :data="chartData"
        index="period"
        :categories="chartCategories"
        :height="200"
        :x-axis="false"
        :y-label="`Average XP per ${selectedBucket.label}`"
        :format="(value: number) => `${format.format(value)} XP / ${selectedBucket.unit}`"
        :aria-label="`Average experience gained per ${selectedBucket.label}`"
        :tooltip="{ content: { ariaLabel: '' } }"
      >
        <template #tooltip="{ title, rows, points }">
          <p class="font-medium">{{ title }}</p>
          <p v-if="points.every((point) => point.yValue === 0)" class="text-muted">No XP recorded for this period.</p>
          <div
            v-for="row in rows.filter((_, index) => points[index]?.yValue !== 0)"
            :key="row.label"
            class="flex items-center gap-2"
          >
            <span class="size-2 shrink-0 rounded-full" :style="{ backgroundColor: row.color }" />
            <span>{{ row.label }}:</span>
            <span class="ml-auto font-medium tabular-nums">{{ row.value }}</span>
          </div>
        </template>
      </UChart>
      <p v-else class="mt-4 text-sm text-gray-500">Select at least one skill to show its history.</p>
      <div class="mt-2 flex justify-between text-xs text-gray-500">
        <span>{{ bars.length ? dateFormat.format(bars[0].time) : '' }} UTC</span>
        <span>{{ bars.length ? dateFormat.format(bars[bars.length - 1].time) : '' }} UTC</span>
      </div>
      <p v-if="rollingHour" class="mt-2 text-xs text-gray-500">One point per minute, showing the average XP rate over the previous five completed minutes.</p>
      <p v-else class="mt-2 text-xs text-gray-500">Each point is an average XP rate. The latest period is still in progress, and empty periods have no recorded gains.</p>
      <p v-if="claimId || locationFilter !== 'all'" class="mt-1 text-xs text-gray-500">Location-filtered XP uses the last observed position at each live XP update. Earlier and disconnected periods may be unavailable; unknown locations are kept separate.</p>
    </template>
  </UCard>
</template>
