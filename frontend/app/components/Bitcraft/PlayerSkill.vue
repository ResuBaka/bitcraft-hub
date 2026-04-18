<script setup lang="ts">
import { rarityToTextClass, tierToBorderClassByLevel } from "~/utils";
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
  xp_info: any;
  skill: any;
  tools: any;
}>();
const numberFormat = new Intl.NumberFormat(undefined);
const tierColor = useTierColor();

const itemForSkill = computed(() => {
  const index = skillToToolIndex[props.skill as keyof typeof skillToToolIndex];

  if (index === undefined || index === null) return null;
  return props.tools?.pockets?.[index]?.contents.item ?? null;
});

const itemIcon = computed(() => {
  if (!itemForSkill.value?.icon_asset_name) return null;
  const icon = iconAssetUrlNameRandom(itemForSkill.value.icon_asset_name);
  return icon.show ? icon.url : null;
});

const levelMap: Record<number, number> = {
  1: 0,
  2: 520,
  3: 1100,
  4: 1740,
  5: 2460,
  6: 3270,
  7: 4170,
  8: 5170,
  9: 6290,
  10: 7540,
  11: 8930,
  12: 10490,
  13: 12220,
  14: 14160,
  15: 16320,
  16: 18730,
  17: 21420,
  18: 24410,
  19: 27760,
  20: 31490,
  21: 35660,
  22: 40310,
  23: 45490,
  24: 51280,
  25: 57740,
  26: 64940,
  27: 72980,
  28: 81940,
  29: 91950,
  30: 103110,
  31: 115560,
  32: 129460,
  33: 144960,
  34: 162260,
  35: 181560,
  36: 203100,
  37: 227130,
  38: 253930,
  39: 283840,
  40: 317220,
  41: 354450,
  42: 396000,
  43: 442350,
  44: 494070,
  45: 551770,
  46: 616150,
  47: 687980,
  48: 768130,
  49: 857560,
  50: 957330,
  51: 1068650,
  52: 1192860,
  53: 1331440,
  54: 1486060,
  55: 1658570,
  56: 1851060,
  57: 2065820,
  58: 2305430,
  59: 2572780,
  60: 2871080,
  61: 3203890,
  62: 3575230,
  63: 3989550,
  64: 4451810,
  65: 4967590,
  66: 5543050,
  67: 6185120,
  68: 6901500,
  69: 7700800,
  70: 8592610,
  71: 9587630,
  72: 10697810,
  73: 11936490,
  74: 13318540,
  75: 14860540,
  76: 16581010,
  77: 18500600,
  78: 20642370,
  79: 23032020,
  80: 25698250,
  81: 28673070,
  82: 31992200,
  83: 35695470,
  84: 39827360,
  85: 44437480,
  86: 49581160,
  87: 55320170,
  88: 61723410,
  89: 68867770,
  90: 76839000,
  91: 85732810,
  92: 95656000,
  93: 106727680,
  94: 119080790,
  95: 132863630,
  96: 148241700,
  97: 165399620,
  98: 184543380,
  99: 205902840,
  100: 229734400,
  101: 256324240,
  102: 285991580,
  103: 319092580,
  104: 356024680,
  105: 397231240,
  106: 443207040,
  107: 494504080,
  108: 551738200,
  109: 615596560,
  110: 686845760,
};

const expUntilNextLevel = (skill: RankType) => {
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
        <p v-if="!['Level', 'Experience'].includes(skill)">
          To next:
          <bitcraft-animated-number
            :value="expUntilNextLevel(xp_info)"
            :formater="numberFormat.format"
          />
        </p>
        <p v-if="!['Level', 'Experience'].includes(skill)">
          To next milestone:
          <bitcraft-animated-number :value="expUntilNextTenLevel" :formater="numberFormat.format" />
        </p>
        <p v-if="!['Experience'].includes(skill)">
          Level: {{ numberFormat.format(xp_info.level ?? 0) }}
        </p>
      </div>
      <div v-if="itemForSkill" class="flex items-center gap-3 rounded-md px-2 py-1 text-right">
        <div class="flex h-13 w-13 items-center justify-center rounded bg-white dark:bg-gray-950">
          <img
            v-if="itemIcon"
            :src="itemIcon"
            :alt="itemForSkill!.name"
            class="h-10 w-10 object-contain"
            loading="lazy"
          />
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
