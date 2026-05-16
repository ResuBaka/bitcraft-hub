<script setup lang="ts">
import { useNow } from "@vueuse/core";
import type { PlayerLeaderboardResponse } from "~/types/PlayerLeaderboardResponse";

const numberFormat = new Intl.NumberFormat(undefined);
const props = defineProps<{
  claimId: number | bigint;
}>();

const { data: leaderboard, pending } = await useFetchMsPack<PlayerLeaderboardResponse>(
  () => {
    return `/api/bitcraft/leaderboard/claims/${props["claimId"]}`;
  },
  {
    lazy: true,
  },
);

const skills = computed(() => {
  if (!leaderboard.value?.leaderboard) {
    return [];
  }

  return Object.keys(leaderboard.value.leaderboard).filter((name) => {
    return name !== "Experience" && name !== "Level";
  });
});

let selectedSkills = ref("Experience");

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
  ];

  for (const skill of skills.value) {
    menu.push({ value: skill, label: skill });
  }

  return menu.map((item) => ({
    ...item,
    icon: icons[item.value]?.icon,
  }));
});

const topics = computed(() => {
  const topicsSet = new Set<string>();

  if (!leaderboard.value?.leaderboard?.[selectedSkills.value]) {
    return [];
  }

  for (const player of leaderboard.value.leaderboard[selectedSkills.value]) {
    if (!player) {
      continue;
    }

    topicsSet.add(`experience:${selectedSkills.value}.${player.player_id.toString()}`);
  }

  return Array.from(topicsSet);
});

const totalExperienceTopics = computed(() => {
  const topicsSet = new Set<string>();

  if (!leaderboard.value?.leaderboard?.Experience) {
    return [];
  }

  for (const player of leaderboard.value.leaderboard.Experience) {
    if (!player) {
      continue;
    }

    topicsSet.add(`total_experience.${player.player_id.toString()}`);
  }

  return Array.from(topicsSet);
});

const toBigInt = (value: number | bigint | undefined) => {
  if (typeof value === "bigint") {
    return value;
  }

  if (typeof value === "number") {
    return BigInt(Math.trunc(value));
  }

  return 0n;
};

const reorderByExperience = (list: Array<{ experience: number | bigint }>, startIndex: number) => {
  if (startIndex < 0 || startIndex >= list.length) {
    return;
  }

  let index = startIndex;

  while (index > 0) {
    const current = toBigInt(list[index]?.experience);
    const prev = toBigInt(list[index - 1]?.experience);

    if (current <= prev) {
      break;
    }

    const temp = list[index - 1];
    list[index - 1] = list[index];
    list[index] = temp;
    index -= 1;
  }

  while (index < list.length - 1) {
    const current = toBigInt(list[index]?.experience);
    const next = toBigInt(list[index + 1]?.experience);

    if (current >= next) {
      break;
    }

    const temp = list[index + 1];
    list[index + 1] = list[index];
    list[index] = temp;
    index += 1;
  }
};

registerWebsocketMessageHandler("Experience", topics, (message) => {
  const list = leaderboard.value?.leaderboard?.[message.skill_name];

  if (!list) {
    return;
  }

  const index = list.findIndex((item) => Number(item?.player_id) === Number(message.user_id));
  if (index === -1) {
    return;
  }

  leaderboard.value.leaderboard[message.skill_name][index].experience = message.experience;
  leaderboard.value.leaderboard[message.skill_name][index].level = message.level;
  triggerRef(leaderboard);
  reorderByExperience(leaderboard.value.leaderboard[message.skill_name], index);
});

registerWebsocketMessageHandler("TotalExperience", totalExperienceTopics, (message) => {
  const list = leaderboard.value?.leaderboard?.Experience;

  if (!list) {
    return;
  }

  const index = list.findIndex((item) => Number(item?.player_id) === Number(message.user_id));
  if (index === -1) {
    return;
  }

  leaderboard.value.leaderboard.Experience[index].experience = message.experience;
  triggerRef(leaderboard);
  reorderByExperience(leaderboard.value.leaderboard.Experience, index);
});

const timeHeader = computed(() => {
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

const columnsForSkill = computed(() => {
  if (selectedSkills.value === "Experience") {
    return [
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
    ];
  }

  if (selectedSkills.value === "Level") {
    return [
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
    ];
  }

  if (selectedSkills.value === "Time Played" || selectedSkills.value === "Time Online") {
    return [
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
    ];
  }

  return [
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
  ];
});
</script>

<template>
  <UContainer class="py-6">
    <div class="flex flex-col gap-4">
      <h2 class="text-xl font-semibold tracking-tight text-center">Claim Leaderboard</h2>

      <USelectMenu
        v-model="selectedSkills"
        class="lg:hidden"
        value-key="value"
        label-key="label"
        :items="skillMenu"
      />

      <div class="hidden lg:grid lg:grid-cols-5 gap-2">
        <UButton
          v-for="skill in skillMenu"
          :key="skill.value"
          :variant="selectedSkills === skill.value ? 'solid' : 'soft'"
          color="neutral"
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
            :columns="columnsForSkill"
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
              <span class="font-medium">{{ row.index + 1 }}</span>
            </template>

            <template #player-cell="{ row }">
              <NuxtLink
                class="font-semibold hover:underline"
                :to="{ path: '/players/' + row.original.player_id }"
              >
                {{ row.original.player_name }}
              </NuxtLink>
            </template>

            <template #experience-cell="{ row }">
              <bitcraft-animated-number
                :value="row.original.experience"
                :speed="8"
                :formater="numberFormat.format"
                color
                animate
              />
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
  </UContainer>
</template>
