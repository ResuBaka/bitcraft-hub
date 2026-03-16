<script setup lang="ts">
import { useNow } from "@vueuse/core";
import type { GetTop100Response } from "~/types/GetTop100Response";
import { watch } from "vue";

const numberFormat = new Intl.NumberFormat(undefined);

const { data: leaderboard, pending } = await useLazyFetchMsPack<GetTop100Response>(
  () => {
    return `/leaderboard`;
  },
  {
    deep: true,
  },
);

const route = useRoute();
const router = useRouter();

const skills = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return [];
  }

  return Object.keys(leaderboard.value?.leaderboard).filter((name) => {
    return name !== "Experience" && name !== "Level" && name !== "Experience Per Hour";
  });
});

let selectedSkills = ref("Experience");
if (route.query.skill) {
  selectedSkills.value = route.query.skill as string;
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

const topics = computed(() => {
  let topicsSet = new Set<string>();

  if (!leaderboard.value?.leaderboard) {
    return [];
  }

  if (!leaderboard.value?.leaderboard[selectedSkills.value]) {
    return [];
  }

  for (const player of leaderboard.value.leaderboard[selectedSkills.value]) {
    if (player === undefined) {
      continue
    }
    topicsSet.add(`experience:${selectedSkills.value}.${player.player_id.toString()}`);
  }

  return Array.from(topicsSet);
});

function swapArrayRank(arr, indexA, indexB) {
  var temp = arr[indexA];
  arr[indexA] = arr[indexB];
  arr[indexB] = temp;
}

registerWebsocketMessageHandler("Experience", topics, (message) => {
  const skill = leaderboard.value?.leaderboard[message.skill_name].findIndex(
    (item) => item.player_id === message.user_id,
  );

  if (skill) {
    leaderboard.value.leaderboard[message.skill_name][skill].experience = message.experience;
    leaderboard.value.leaderboard[message.skill_name][skill].experience_per_hour =
      message.experience_per_hour;
    if (
      Number(leaderboard.value.leaderboard[message.skill_name][skill].rank) !== Number(message.rank)
    ) {
      swapArrayRank(
        leaderboard.value.leaderboard[message.skill_name],
        skill.rank - 1,
        message.rank - 1,
      );
    }
  }
});

const totalExperienceTopics = computed(() => {
  if (!leaderboard.value?.leaderboard || selectedSkills.value !== "Experience") {
    return [];
  }

  let topicsSet = new Set<string>();

  for (const player of leaderboard.value.leaderboard["Experience"]) {
    if (player === undefined) {
      continue
    }
    topicsSet.add(`total_experience.${player.player_id.toString()}`);
  }

  return Array.from(topicsSet);
});

registerWebsocketMessageHandler("TotalExperience", totalExperienceTopics, (message) => {
  const skill = leaderboard.value?.leaderboard["Experience"].findIndex((item) => {
    if (item === undefined) {
      return false
    }

    return item.player_id === message.user_id
  });

  if (skill) {
    leaderboard.value.leaderboard["Experience"][skill].experience = message.experience;
    leaderboard.value.leaderboard["Experience"][skill].experience_per_hour =
      message.experience_per_hour;
    if (leaderboard.value.leaderboard["Experience"][skill].rank !== message.rank) {
      swapArrayRank(leaderboard.value.leaderboard["Experience"], skill.rank - 1, message.rank - 1);
    }

    leaderboard.value.leaderboard["Experience"][skill].rank = message.rank;
  }
});

const playerStateTopics = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return [];
  }

  let topicsSet = new Set<string>();

  for (const player of Object.values(leaderboard.value.player_map)) {
    topicsSet.add(`player_state.${player.entity_id.toString()}`);
  }

  return Array.from(topicsSet);
});

registerWebsocketMessageHandler("PlayerState", playerStateTopics, (message) => {
  leaderboard.value.player_map[message.entity_id] = message;
});

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

const totelExperiencePerHourAverage = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return 0;
  }

  let totalExperience = 0;

  totalExperience += leaderboard.value.leaderboard["Experience"].reduce((acc, curr) => {
    return acc + curr.experience_per_hour;
  }, 0);

  return Math.ceil(totalExperience / leaderboard.value.leaderboard["Experience"].length);
});

const experiencePerHourAverage = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return 0;
  }

  let totalExperience = 0;

  totalExperience += leaderboard.value.leaderboard["Experience Per Hour"].reduce((acc, curr) => {
    return acc + curr.experience;
  }, 0);

  return Math.ceil(totalExperience / leaderboard.value.leaderboard["Experience Per Hour"].length);
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
const game_start = new Date("2026-02-26T17:00:00Z");

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
  { id: "rank", header: "Rank" },
  { id: "player", header: "Player" },
  // {
  //   id: "experiencePerHour",
  //   header: `Experience/h ${numberFormat.format(totelExperiencePerHourAverage.value)}`,
  //   meta: { class: { th: "text-right", td: "text-right" } },
  // },
  {
    id: "experience",
    header: "Experience",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
]);

const experiencePerHourColumns = computed(() => [
  { id: "rank", header: "Rank" },
  { id: "player", header: "Player" },
  {
    id: "experienceValue",
    header: `Experience/h ${numberFormat.format(experiencePerHourAverage.value)}`,
    meta: { class: { th: "text-right", td: "text-right" } },
  },
]);

const levelColumns = computed(() => [
  { id: "rank", header: "Rank" },
  { id: "player", header: "Player" },
  {
    id: "level",
    header: "Level",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
]);

const timeColumns = computed(() => [
  { id: "rank", header: "Rank" },
  { id: "player", header: "Player", meta: { class: { th: "text-center" } } },
  {
    id: "time",
    header: timeHeader.value,
    meta: { class: { th: "text-right", td: "text-right" } },
  },
]);

const defaultColumns = computed(() => [
  { id: "rank", header: "Rank" },
  { id: "player", header: "Player" },
  {
    id: "level",
    header: "Level",
    meta: { class: { th: "text-center", td: "text-center" } },
  },
  {
    id: "experience",
    header: "Experience",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
]);

const playerLinkClass = (playerId: number) => {
  const signedIn = leaderboard.value?.player_map?.[playerId]?.signed_in;
  return [
    "font-semibold hover:underline",
    signedIn ? "text-emerald-600 dark:text-emerald-400" : "text-gray-900 dark:text-gray-100",
  ].join(" ");
};

const columnsForSkill = (skill: string) => {
  if (skill === "Experience") {
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
  <UContainer class="w-full max-w-none py-6">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-4">
        <div class="flex flex-col gap-3">
          <div class="flex flex-col items-center gap-3 text-center">
            <h1 class="text-2xl font-semibold tracking-tight">Leaderboards</h1>
          </div>
        </div>

        <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
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

        <div class="rounded-lg border border-gray-200 shadow-sm dark:border-gray-800">
          <div class="w-full overflow-x-auto">
            <UTable
              :key="selectedSkills"
              :columns="columnsForSkill(selectedSkills)"
              :data="leaderboard?.leaderboard?.[selectedSkills] || []"
              :loading="pending"
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
              </template>
              <template #player-cell="{ row }">
                <NuxtLink
                  :to="{ path: 'players/' + row.original.player_id }"
                  :class="playerLinkClass(row.original.player_id)"
                >
                  {{ row.original.player_name }}
                </NuxtLink>
              </template>
              <template #experiencePerHour-cell="{ row }">
                {{ numberFormat.format(row.original.experience_per_hour) }}
              </template>
              <template #experience-cell="{ row }">
                <bitcraft-animated-number
                  :value="row.original.experience"
                  :speed="8"
                  :formater="numberFormat.format"
                  color
                  :animate="animateNumbers"
                />
              </template>
              <template #experienceValue-cell="{ row }">
                {{ numberFormat.format(row.original.experience) }}
              </template>
              <template #level-cell="{ row }">
                {{ numberFormat.format(row.original.level) }}
              </template>
              <template #time-cell="{ row }">
                {{ secondsToDaysMinutesSecondsFormat(row.original.time_played) }}
              </template>
            </UTable>
          </div>
        </div>
      </div>
    </div>
  </UContainer>
</template>
