<script setup lang="ts">
import { watchDebounced } from "@vueuse/shared";
import { registerWebsocketMessageHandler } from "~/composables/websocket";

const props = defineProps<{ playerId?: string; claimId?: string }>();
type Point = { day: string; experience_gained: number | bigint; skill_id?: number };
type Skill = { id: number; name: string };
type Scale = "hour" | "day" | "week" | "month";

const scale = ref<Scale>("hour");
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

const format = new Intl.NumberFormat();
const dateFormat = new Intl.DateTimeFormat(undefined, {
  month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", timeZone: "UTC",
});
const bars = computed(() => {
  const interval = scale.value === "hour" ? 5 * 60_000 : scale.value === "day" ? 30 * 60_000 : scale.value === "week" ? 3 * 3_600_000 : 7 * 86_400_000;
  const duration = scale.value === "hour" ? 3_600_000 : (scale.value === "day" ? 1 : scale.value === "week" ? 7 : 180) * 86_400_000;
  // Weekly TimescaleDB buckets start on Monday, 1970-01-05 UTC.
  const origin = scale.value === "month" ? 4 * 86_400_000 : 0;
  const bucket = (time: number) => Math.floor((time - origin) / interval) * interval + origin;
  const now = Date.now();
  const values = new Map<string, number>();
  for (const point of data.value ?? []) {
    const time = Date.parse(point.day);
    const xp = Number(point.experience_gained);
    values.set(`${time}:${point.skill_id}`, xp);
    values.set(`${time}:total`, (values.get(`${time}:total`) ?? 0) + xp);
  }
  const result: { time: number; [key: string]: number }[] = [];
  for (let time = bucket(now - duration); time <= bucket(now); time += interval) {
    const row: { time: number; [key: string]: number } = { time };
    for (const skill of chartSkills.value) row[`skill_${skill.id}`] = values.get(`${time}:${skill.id}`) ?? 0;
    row.total = values.get(`${time}:total`) ?? 0;
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
  period: `${dateFormat.format(bar.time)} UTC`,
})));
</script>

<template>
  <UCard :ui="{ body: 'p-4' }">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h2 class="text-lg font-semibold">Experience history</h2>
        <p class="text-xs text-gray-500">UTC · XP changes since tracking began</p>
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
        :format="(value: number) => format.format(value)"
        aria-label="Experience gained by time period"
      />
      <p v-else class="mt-4 text-sm text-gray-500">Select at least one skill to show its history.</p>
      <div class="mt-2 flex justify-between text-xs text-gray-500">
        <span>{{ bars.length ? dateFormat.format(bars[0].time) : '' }} UTC</span>
        <span>{{ bars.length ? dateFormat.format(bars[bars.length - 1].time) : '' }} UTC</span>
      </div>
      <p class="mt-2 text-xs text-gray-500">Hover over the chart for XP changes. Empty periods have no recorded changes.</p>
      <p v-if="claimId || locationFilter !== 'all'" class="mt-1 text-xs text-gray-500">Location-filtered XP uses the last observed position at each live XP update. Earlier and disconnected periods may be unavailable; unknown locations are kept separate.</p>
    </template>
  </UCard>
</template>
