<script setup lang="ts">
import { useNow } from "@vueuse/core";
import type { LeaderboardBoard } from "~/types/LeaderboardBoard";
import type { LeaderboardBoardRow } from "~/types/LeaderboardBoardRow";
import type { LeaderboardBoardSnapshot } from "~/types/LeaderboardBoardSnapshot";
import type { LeaderboardBoardUpdate } from "~/types/LeaderboardBoardUpdate";
import { acceptsLeaderboardSnapshot, applyLeaderboardDelta } from "~/utils/leaderboardLive";
import { watch } from "vue";

const numberFormat = new Intl.NumberFormat(undefined);

const { data: boards } = await useFetchMsPack<LeaderboardBoard[]>("/leaderboard/boards");

const route = useRoute();
const router = useRouter();

const summaryBoards = new Set(["Experience", "Level", "Experience Per Hour", "Skills", "Professions"]);
const skills = computed(() =>
  (boards.value ?? []).map((board) => board.name).filter((name) => !summaryBoards.has(name)),
);

const selectedSkills = ref(typeof route.query.skill === "string" ? route.query.skill : "Professions");
const selectedBoardId = computed(() => boards.value?.find((board) => board.name === selectedSkills.value)?.id ?? -1);
const boardTopic = computed(() => `leaderboard_delta.${selectedBoardId.value}`);
const { data: snapshot, pending } = await useLazyFetchMsPack<LeaderboardBoardSnapshot>(
  () => `/leaderboard/boards/${selectedBoardId.value}`,
);
const liveBoard = shallowRef<LeaderboardBoardUpdate>();
const rows = computed(() => {
  const id = selectedBoardId.value;
  if (liveBoard.value?.board_id === id) return liveBoard.value.rows;
  return snapshot.value?.board_id === id ? snapshot.value.rows : [];
});

type LeaderboardSnapshots = {
  board: string;
  entries: { player_id: number | bigint; rank: number | bigint; captured_at: string }[];
};
const snapshotPlayerIds = computed(() =>
  rows.value.map((entry) => entry.player_id).sort().join(","),
);
const { data: snapshots, refresh: refreshSnapshots } = useLazyFetchMsPack<LeaderboardSnapshots>(
  () => `/leaderboard/rank-snapshots?board=${encodeURIComponent(selectedSkills.value)}&players=${snapshotPlayerIds.value}`,
  { server: false },
);
const snapshotsByPlayer = computed(() => new Map(
  snapshots.value?.board === selectedSkills.value
    ? snapshots.value.entries.map((entry) => [String(entry.player_id), entry])
    : [],
));
function rankChange(playerId: string, rank: number) {
  const snapshot = snapshotsByPlayer.value.get(String(playerId));
  return snapshot ? Number(snapshot.rank) - rank : null;
}
function snapshotTitle(playerId: string) {
  const snapshot = snapshotsByPlayer.value.get(String(playerId));
  if (!snapshot) return undefined;
  const date = new Date(snapshot.captured_at);
  const time = Number.isNaN(date.getTime()) ? "unknown time" : new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium", timeStyle: "short", timeZone: "UTC",
  }).format(date) + " UTC";
  return `Previously #${numberFormat.format(snapshot.rank)} · Snapshot ${time}`;
}

const animateNumbers = ref(true);

watch(selectedSkills, (newValue) => {
  let currentQuery = route.query;
  router.push({ query: { ...currentQuery, skill: newValue } });
});

watch(selectedSkills, async () => {
  animateNumbers.value = false;
  await nextTick();
  animateNumbers.value = true;
});

const websocket = useWebsocketStore();
let needsSync = true;
let lastGapSyncAt = 0;

function requestBoardSync() {
  const boardId = selectedBoardId.value;
  const current = liveBoard.value?.board_id === boardId ? liveBoard.value : undefined;
  websocket.sendMessage({
    t: "SyncLeaderboardBoard",
    c: { board_id: boardId, epoch: current?.epoch ?? null, revision: current?.revision ?? null },
  });
}

// The delta handler owns the topic. Reset frames use the same subscription.
registerWebsocketMessageHandler("LeaderboardBoardReset", [], (message) => {
  if (!acceptsLeaderboardSnapshot(selectedBoardId.value, liveBoard.value, message)) return;
  liveBoard.value = message;
  lastGapSyncAt = 0;
});
registerWebsocketMessageHandler("LeaderboardBoardDelta", boardTopic, (message) => {
  const result = applyLeaderboardDelta(selectedBoardId.value, liveBoard.value, message);
  if (result.status === "applied") {
    liveBoard.value = result.board;
    lastGapSyncAt = 0;
  } else if (result.status === "resync" && Date.now() - lastGapSyncAt >= 500) {
    lastGapSyncAt = Date.now();
    requestBoardSync();
  }
});

registerWebsocketMessageHandler("SubscribedTopics", [], (topics) => {
  if (!needsSync || !topics.includes(boardTopic.value)) return;
  needsSync = false;
  requestBoardSync();
});
watch(() => websocket.isConnected, (connected) => {
  if (!connected) {
    // Keep the last revision for a cheap resume on reconnect.
    needsSync = true;
    lastGapSyncAt = 0;
  }
});
function requestSubscriptionStatus() {
  if (websocket.isConnected) websocket.sendMessage({ t: "ListSubscribedTopics" });
}
watch(boardTopic, () => {
  liveBoard.value = undefined;
  needsSync = true;
  lastGapSyncAt = 0;
  requestSubscriptionStatus();
});
onMounted(requestSubscriptionStatus);

// Rank comparison snapshots change once per UTC day, not every 30 seconds.
let snapshotTimer: ReturnType<typeof setTimeout> | undefined;
function scheduleSnapshotRefresh() {
  const now = new Date();
  const nextMidnight = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate() + 1);
  snapshotTimer = setTimeout(() => {
    void refreshSnapshots({ dedupe: "defer" });
    scheduleSnapshotRefresh();
  }, nextMidnight - now.getTime() + 60_000);
}
onMounted(scheduleSnapshotRefresh);
onUnmounted(() => { if (snapshotTimer) clearTimeout(snapshotTimer); });

const icons = {
  Fishing: { icon: "i-mdi-fish", class: "text-blue-500" },
  Mining: { icon: "i-mdi-pickaxe", class: "text-gray-500" },
  Woodcutting: { icon: "i-mdi-forest", class: "text-emerald-600" },
  Farming: { icon: "i-mdi-sprout", class: "text-emerald-600" },
  Carpentry: { icon: "i-mdi-hand-saw", class: "text-amber-700" },
  Foraging: { icon: "i-mdi-leaf", class: "text-emerald-600" },
  Forestry: { icon: "i-mdi-axe", class: "text-amber-700" },
  Masonry: { icon: "i-mdi-screwdriver", class: "text-gray-500" },
  Smithing: { icon: "i-mdi-anvil", class: "text-gray-500" },
  Scholar: { icon: "i-mdi-school", class: "text-sky-600" },
  Hunting: { icon: "i-mdi-bow-arrow", class: "text-slate-600" },
  Cooking: { icon: "i-mdi-stove", class: "text-orange-600" },
  Leatherworking: { icon: "i-mdi-bag-personal", class: "text-amber-700" },
  Tailoring: { icon: "i-mdi-tshirt-crew", class: "text-indigo-600" },
  Sailing: { icon: "i-mdi-sail-boat", class: "text-sky-600" },
  Slayer: { icon: "i-mdi-sword", class: "text-rose-600" },
  Taming: { icon: "i-mdi-paw", class: "text-amber-600" },
  Construction: { icon: "i-mdi-hard-hat", class: "text-yellow-600" },
  "Time Online": { icon: "i-mdi-timer-outline", class: "text-cyan-600" },
  "Time Played": { icon: "i-mdi-clock-outline", class: "text-cyan-600" },
  Experience: { icon: "i-mdi-star-circle", class: "text-amber-500" },
  Level: { icon: "i-mdi-chart-line", class: "text-emerald-600" },
};

const experiencePerHourAverage = computed(() => {
  if (!rows.value.length) return 0;
  const total = rows.value.reduce((sum, row) => sum + Number(row.experience ?? 0), 0);
  return Math.ceil(total / rows.value.length);
});

const secondsToDaysMinutesSecondsFormat = (seconds: number) => {
  const days = Math.floor(seconds / (60 * 60 * 24));
  const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  const minutes = Math.floor((seconds % (60 * 60)) / 60);
  const secondsLeft = seconds % 60;

  let result = "";

  if (days > 0) {
    result += `${days}d `;
  }

  if (hours > 0) {
    result += `${hours}h `;
  }

  if (minutes > 0) {
    result += `${minutes}m `;
  }

  if (secondsLeft > 0) {
    result += `${secondsLeft}s`;
  }

  return result;
};

const now = useNow({ interval: 1000, controls: true });
const game_start = new Date("2026-11-13T15:09:56Z");

const countDownUntilResearchIsFinished = computed(() => {
  const diff = now.now.value.getTime() - game_start.getTime();

  return {
    days: Math.floor(diff / (1000 * 60 * 60 * 24)),
    hours: Math.floor((diff / (1000 * 60 * 60)) % 24),
    minutes: Math.floor((diff / 1000 / 60) % 60),
    seconds: Math.floor((diff / 1000) % 60),
  };
});

const skillMenu = computed(() => {
  const menu = [
    { value: "Professions", label: "Professions experience" },
    { value: "Skills", label: "Skills experience" },
    { value: "Experience", label: "Total experience" },
    { value: "Level", label: "Total level" },
    // { value: "Experience Per Hour", label: "Experience Per Hour" },
  ];

  for (const skill of skills.value) {
    menu.push({ value: skill, label: skill });
  }

  return menu.map((item) => ({
    ...item,
    icon: icons[item.value]?.icon,
  }));
});

const isHydrated = ref(false);

onMounted(() => {
  isHydrated.value = true;
});

const timeHeader = computed(() => {
  if (!isHydrated.value) {
    return "Time";
  }

  const parts = [] as string[];
  if (countDownUntilResearchIsFinished.value.days) {
    parts.push(`${countDownUntilResearchIsFinished.value.days}d`);
  }
  if (countDownUntilResearchIsFinished.value.hours) {
    parts.push(`${countDownUntilResearchIsFinished.value.hours}h`);
  }
  if (countDownUntilResearchIsFinished.value.minutes) {
    parts.push(`${countDownUntilResearchIsFinished.value.minutes}m`);
  }
  if (countDownUntilResearchIsFinished.value.seconds) {
    parts.push(`${countDownUntilResearchIsFinished.value.seconds}s`);
  }

  if (parts.length === 0) {
    return "Time";
  }

  return `Time (Game is online since ${parts.join(" ")})`;
});

const experienceColumns = computed(() => [
  {
    id: "rank",
    header: "Rank",
    meta: { class: { th: "w-20 text-center", td: "w-20 text-center" } },
  },
  {
    id: "player",
    header: "Player",
    meta: { class: { th: "w-auto text-center", td: "w-auto text-center" } },
  },
  {
    id: "experience",
    header: "Experience",
    meta: { class: { th: "w-48 text-right", td: "w-48 text-right" } },
  },
]);

const experiencePerHourColumns = computed(() => [
  {
    id: "rank",
    header: "Rank",
    meta: { class: { th: "w-20 text-center", td: "w-20 text-center" } },
  },
  {
    id: "player",
    header: "Player",
    meta: { class: { th: "w-auto text-center", td: "w-auto text-center" } },
  },
  {
    id: "experienceValue",
    header: `Experience/h ${numberFormat.format(experiencePerHourAverage.value)}`,
    meta: { class: { th: "w-48 text-right", td: "w-48 text-right" } },
  },
]);

const levelColumns = computed(() => [
  {
    id: "rank",
    header: "Rank",
    meta: { class: { th: "w-20 text-center", td: "w-20 text-center" } },
  },
  {
    id: "player",
    header: "Player",
    meta: { class: { th: "w-auto text-center", td: "w-auto text-center" } },
  },
  {
    id: "level",
    header: "Level",
    meta: { class: { th: "w-48 text-right", td: "w-48 text-right" } },
  },
]);

const timeColumns = computed(() => [
  {
    id: "rank",
    header: "Rank",
    meta: { class: { th: "w-20 text-center", td: "w-20 text-center" } },
  },
  {
    id: "player",
    header: "Player",
    meta: { class: { th: "w-auto text-center", td: "w-auto text-center" } },
  },
  {
    id: "time",
    header: timeHeader.value,
    meta: { class: { th: "w-100 text-right", td: "w-100 text-right" } },
  },
]);

const defaultColumns = computed(() => [
  {
    id: "rank",
    header: "Rank",
    meta: { class: { th: "w-20 text-center", td: "w-20 text-center" } },
  },
  {
    id: "player",
    header: "Player",
    meta: { class: { th: "w-auto text-center", td: "w-auto text-center" } },
  },
  {
    id: "level",
    header: "Level",
    meta: { class: { th: "w-30 text-center", td: "w-30 text-center" } },
  },
  {
    id: "experience",
    header: "Experience",
    meta: { class: { th: "w-30 text-right", td: "w-30 text-right" } },
  },
]);

const playerLinkClass = (row: LeaderboardBoardRow) => {
  return [
    "font-semibold hover:underline",
    row.signed_in ? "text-emerald-600 dark:text-emerald-400" : "text-gray-900 dark:text-gray-100",
  ].join(" ");
};

const columnsForSkill = (skill: string) => {
  if (skill === "Experience" || skill === "Professions" || skill === "Skills") {
    return experienceColumns.value;
  }

  if (skill === "Experience Per Hour") {
    return experiencePerHourColumns.value;
  }

  if (skill === "Level") {
    return levelColumns.value;
  }

  if (skill === "Time Played" || skill === "Time Online") {
    return timeColumns.value;
  }

  return defaultColumns.value;
};
</script>

<template>
  <UContainer class="py-6">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-4">
        <div class="flex flex-col lg:flex-row gap-4">
          <div class="flex flex-col gap-2 lg:w-70 shrink-0">
            <div class="flex flex-col gap-3">
              <div class="flex flex-col items-center gap-3 text-center">
                <h1 class="text-2xl font-semibold tracking-tight">Leaderboards</h1>
              </div>
            </div>
            <UButton
              v-for="skill in skillMenu"
              :key="skill.value"
              :variant="selectedSkills === skill.value ? 'solid' : 'soft'"
              color="neutral"
              size="md"
              block
              @click="selectedSkills = skill.value"
            >
              <template #leading>
                <UIcon
                  v-if="skill.icon"
                  :name="skill.icon"
                  :class="icons[skill.value]?.class || 'text-gray-500'"
                />
              </template>
              {{ skill.label }}
            </UButton>
          </div>

          <div class="rounded-lg border border-gray-200 shadow-sm dark:border-gray-800 flex-1">
            <div class="w-full overflow-x-auto">
              <UTable
                :key="selectedSkills"
                :columns="columnsForSkill(selectedSkills)"
                :data="rows"
                :loading="pending && rows.length === 0"
                :watch-options="{ deep: false }"
                loading-color="neutral"
                loading-animation="carousel"
                :meta="{
                  class: {
                    tr: (row) => {
                      return 'hover:bg-neutral-200 dark:hover:bg-neutral-700';
                    },
                  },
                }"
              >
                <template #rank-cell="{ row }">
                  <span class="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {{ row.index + 1 }}
                  </span>
                  <span v-if="rankChange(row.original.player_id, row.index + 1) !== null" class="ml-1.5 text-xs font-medium" :class="rankChange(row.original.player_id, row.index + 1)! > 0 ? 'text-green-600 dark:text-green-400' : rankChange(row.original.player_id, row.index + 1)! < 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-500'" :title="snapshotTitle(row.original.player_id)">
                    <template v-if="rankChange(row.original.player_id, row.index + 1)! > 0">▲ {{ numberFormat.format(rankChange(row.original.player_id, row.index + 1)!) }}</template>
                    <template v-else-if="rankChange(row.original.player_id, row.index + 1)! < 0">▼ {{ numberFormat.format(-rankChange(row.original.player_id, row.index + 1)!) }}</template>
                    <template v-else>—</template>
                  </span>
                </template>
                <template #player-cell="{ row }">
                  <NuxtLink
                    :to="{ path: 'players/' + row.original.player_id }"
                    :class="playerLinkClass(row.original)"
                  >
                    <BitcraftPlayerName :player-id="row.original.player_id">{{ row.original.player_name }}</BitcraftPlayerName>
                  </NuxtLink>
                </template>
                <template #experience-cell="{ row }">
                  <bitcraft-animated-number
                    :value="Number(row.original.experience ?? 0)"
                    :speed="8"
                    :formater="numberFormat.format"
                    color
                    :animate="animateNumbers"
                  />
                </template>
                <template #experienceValue-cell="{ row }">
                  {{ numberFormat.format(row.original.experience ?? 0) }}
                </template>
                <template #level-cell="{ row }">
                  {{ numberFormat.format(row.original.level ?? 0) }}
                </template>
                <template #time-cell="{ row }">
                  {{ secondsToDaysMinutesSecondsFormat(Number(row.original.time_played ?? 0)) }}
                </template>
              </UTable>
            </div>
          </div>
        </div>
      </div>
    </div>
  </UContainer>
</template>
