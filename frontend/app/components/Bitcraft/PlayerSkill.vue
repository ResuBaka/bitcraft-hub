<script setup lang="ts">
import { rarityToTextClass, tierToBorderClassByLevel } from "~/utils";
import type { ResolvedInventory } from "~/types/ResolvedInventory";
import type { LeaderboardSkill } from "~/types/LeaderboardSkill";
import { levelMap } from "~/consts/exp.ts";

const skillToToolIndex = {
  Carpentry: 1,
  Construction: 13,
  Cooking: 10,
  Experience: undefined,
  Farming: 8,
  Fishing: 9,
  Foraging: 11,
  Forestry: 0,
  Hunting: 6,
  Leatherworking: 5,
  Level: undefined,
  Masonry: 2,
  Mining: 3,
  Scholar: 12,
  Slayer: 15,
  Smithing: 4,
  Tailoring: 7,
};

const props = defineProps<{
  xp_info: LeaderboardSkill;
  skill: keyof typeof skillToToolIndex;
  tools: ResolvedInventory;
}>();
const numberFormat = new Intl.NumberFormat(undefined);
const tierColor = useTierColor();

const itemForSkill = computed(() => {
  const index = skillToToolIndex[props.skill as keyof typeof skillToToolIndex];

  if (index === undefined || index === null) return null;
  return props.tools?.pockets?.[index]?.contents?.item ?? null;
});

const itemIcon = computed(() => {
  if (!itemForSkill.value?.icon_asset_name) return null;
  const icon = iconAssetUrlNameRandom(itemForSkill.value.icon_asset_name);
  return icon.show ? icon.url : null;
});

const expUntilNextLevel = (skill: LeaderboardSkill) => {
  const currentLevel = skill.level ?? 0;
  const currentExperience = skill.experience ?? 0;
  const nextLevel = currentLevel + 1;
  const nextLevelExperience = levelMap[nextLevel] ?? 0;
  return Math.max(0, nextLevelExperience - currentExperience);
};

const nextTenLevel = computed(() => {
  const currentLevel = props.xp_info.level ?? 0;
  return Math.ceil(currentLevel / 10) * 10 + (currentLevel % 10 === 0 ? 10 : 0);
});

const expUntilNextTenLevel = computed(() => {
  const currentLevel = props.xp_info.level ?? 0;
  const currentExperience = props.xp_info.experience ?? 0;

  if (currentLevel <= 0) {
    return 0;
  }
  const nextTenLevelExperience = levelMap[nextTenLevel.value] ?? 0;

  return Math.max(0, nextTenLevelExperience - currentExperience);
});
</script>

<template>
  <div
    class="rounded-lg bg-gray-200 p-3 dark:bg-zinc-900 border-l-4"
    :class="tierToBorderClassByLevel(xp_info.level ?? 0)"
  >
    <div class="flex items-center justify-between gap-2">
      <div class="min-w-0">
        <p class="text-sm font-semibold text-gray-900 dark:text-gray-100">
          {{ skill }}
        </p>
      </div>
      <UBadge color="neutral" variant="soft">
        Rank #{{ numberFormat.format(xp_info.rank) }}
      </UBadge>
    </div>
    <div
      class="mt-2 flex flex-wrap items-start justify-between gap-3 text-xs text-gray-500 dark:text-gray-400"
    >
      <div class="space-y-1">
        <p v-if="!['Level'].includes(skill)">
          Experience:
          <bitcraft-animated-number :value="xp_info.experience" :formater="numberFormat.format" />
        </p>
        <p v-if="!['Level', 'Experience', 'Skills', 'Professions'].includes(skill)">
          To next:
          <bitcraft-animated-number
            :value="expUntilNextLevel(xp_info)"
            :formater="numberFormat.format"
          />
        </p>
        <p v-if="!['Level', 'Experience', 'Skills', 'Professions'].includes(skill)">
          To next milestone:
          <bitcraft-animated-number :value="expUntilNextTenLevel" :formater="numberFormat.format" />
        </p>
        <p v-if="!['Experience', 'Skills', 'Professions'].includes(skill)">
          Level: {{ numberFormat.format(xp_info.level ?? 0) }}
        </p>
      </div>
      <div v-if="itemForSkill" class="flex items-center gap-3 rounded-md px-2 py-1 text-right">
        <div class="flex h-13 w-13 items-center justify-center rounded bg-white dark:bg-gray-950">
          <picture v-if="itemIcon">
            <source :srcset="`${itemIcon}.jxl`" type="image/jxl" />
            <source :srcset="`${itemIcon}.avif`" type="image/avif" />
            <img
              :src="`${itemIcon}.webp`"
              :alt="itemForSkill!.name"
              class="h-10 w-10 object-contain"
              loading="lazy"
            />
          </picture>
          <UIcon v-else name="i-lucide-wrench" class="h-6 w-6 text-gray-400" />
        </div>
        <div class="flex flex-col items-end gap-1">
          <div class="flex items-center gap-2">
            <UBadge color="neutral" variant="soft">Tool</UBadge>
            <span
              v-if="itemForSkill?.tier"
              class="text-xs font-semibold leading-none"
              :class="tierColor[itemForSkill!.tier]"
            >
              T{{ itemForSkill!.tier }}
            </span>
          </div>
          <p
            class="text-xs dark:text-gray-300"
            :class="rarityToTextClass(itemForSkill.rarity ?? null)"
          >
            {{ itemForSkill?.name }}
          </p>
          <p v-if="itemForSkill.rarity" class="text-[10px] uppercase tracking-wide text-gray-400">
            {{ itemForSkill?.rarity }}
          </p>
        </div>
      </div>
    </div>
  </div>
</template>
