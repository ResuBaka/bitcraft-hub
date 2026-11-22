<script setup lang="ts">
import InventoryImg from "./InventoryImg.vue";
import ItemHint from "./ItemHint.vue";
import { tierToBorderClassByLevel } from "~/utils";
import type { ResolvedInventory } from "~/types/ResolvedInventory";
import type { LeaderboardSkill } from "~/types/LeaderboardSkill";
import type { PlayerEquipmentSlot } from "~/types/PlayerEquipmentSlot";
import type { PlayerEquipmentStat } from "~/types/PlayerEquipmentStat";
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
  tools?: ResolvedInventory;
  equipmentSlots?: PlayerEquipmentSlot[];
  toolPowers?: Record<number, number | undefined>;
  previousRank?: number | bigint;
  snapshotRank?: number | bigint | null;
}>();
const numberFormat = new Intl.NumberFormat(undefined);
const bonusFormat = new Intl.NumberFormat(undefined, { signDisplay: "exceptZero", maximumFractionDigits: 2 });
const NuxtLink = resolveComponent("NuxtLink");
const currentRank = computed(() => Number(props.snapshotRank ?? props.xp_info.rank));
const rankChange = computed(() => props.previousRank == null || currentRank.value <= 0
  ? null
  : Number(props.previousRank) - currentRank.value);

const gearTiles = computed(() => {
  const toolIndex = skillToToolIndex[props.skill];
  if (toolIndex === undefined && !props.equipmentSlots?.length) return [];
  const tool = toolIndex === undefined ? null : props.tools?.pockets[toolIndex]?.contents;
  const professionItem = (type: "Charm" | "Instrument") =>
    props.equipmentSlots?.find((slot) => slot.slot_type.endsWith(type))?.item;
  const charm = professionItem("Charm");
  const instrument = professionItem("Instrument");
  const power = tool?.item_type === "Item" ? props.toolPowers?.[tool.item_id] : undefined;
  const bonus = (entry: PlayerEquipmentStat) => ({
    label: entry.stat.replace(/([a-z0-9])([A-Z])/g, "$1 $2"),
    value: entry.is_pct ? `${bonusFormat.format(entry.value * 100)}%` : bonusFormat.format(entry.value),
  });
  return [
    { label: "Tool", icon: "i-lucide-wrench", item: tool?.item, id: tool?.item_id, type: tool?.item_type, stats: power == null ? [] : [{ label: "Tool Power", value: numberFormat.format(power) }] },
    { label: "Charm", icon: "i-lucide-gem", item: charm, id: charm?.item_id, type: charm?.item_type, stats: charm?.stats?.map(bonus) ?? [] },
    { label: "Instrument", icon: "i-lucide-music", item: instrument, id: instrument?.item_id, type: instrument?.item_type, stats: instrument?.stats?.map(bonus) ?? [] },
  ].map(({ label, icon, item, id, type, stats }) => ({
    label,
    icon,
    item,
    stats,
    description: `${label}: ${item?.name || (item ? `Unknown item (${id})` : "Empty")}`,
    colors: {
      "--gear-tier": item?.tier != null ? `rgb(var(--tier-${item.tier}, var(--tier-0)))` : undefined,
      "--gear-rarity": item?.rarity && item.rarity !== "Default"
        ? `rgb(var(--rarity-${item.rarity.toLowerCase()}))`
        : undefined,
    },
    to: item?.name && id != null && type
      ? { name: "items-type-id", params: { id, type } }
      : undefined,
  }));
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
      <div class="flex items-center gap-1.5">
        <UBadge color="neutral" variant="soft">Rank #{{ numberFormat.format(currentRank) }}</UBadge>
        <span v-if="rankChange !== null" class="text-xs font-medium" :class="rankChange > 0 ? 'text-green-600 dark:text-green-400' : rankChange < 0 ? 'text-red-600 dark:text-red-400' : 'text-gray-500'">
          <template v-if="rankChange > 0">▲ {{ numberFormat.format(rankChange) }}</template>
          <template v-else-if="rankChange < 0">▼ {{ numberFormat.format(-rankChange) }}</template>
          <template v-else>—</template>
        </span>
      </div>
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
      <div v-if="gearTiles.length" class="flex gap-1.5" role="group" :aria-label="`${skill} equipment`">
        <ItemHint
          v-for="tile in gearTiles"
          :key="tile.label"
          :label="tile.label"
          :item="tile.item"
          :stats="tile.stats"
        >
          <component
            :is="tile.to ? NuxtLink : 'div'"
            :to="tile.to"
            :aria-label="tile.description"
            :role="tile.to ? undefined : 'img'"
            :tabindex="tile.to ? undefined : 0"
            class="gear-tile grid size-11 shrink-0 place-items-center rounded-md border-2 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
            :style="tile.colors"
          >
            <InventoryImg
              v-if="tile.item"
              :key="tile.item.icon_asset_name ?? tile.description"
              :item="{ name: tile.item.name ?? '', icon_asset_name: tile.item.icon_asset_name ?? '' }"
              :width="27"
              :height="27"
              skip-error-text
              aria-hidden="true"
            />
            <UIcon v-else :name="tile.icon" class="size-3.5 text-muted opacity-65" aria-hidden="true" />
          </component>
        </ItemHint>
      </div>
    </div>
  </div>
</template>

<style scoped>
.gear-tile {
  background-color: var(--ui-bg-elevated);
  background-color: color-mix(in srgb, var(--gear-tier, var(--ui-bg-elevated)) 14%, var(--ui-bg-elevated));
  border-color: var(--ui-border-accented);
  border-color: color-mix(in srgb, var(--gear-rarity, var(--ui-text-muted)) 75%, var(--ui-border-accented));
  transition: background-color 150ms ease, border-color 150ms ease;
}

.gear-tile:hover,
.gear-tile:focus-visible {
  background-color: color-mix(in srgb, var(--gear-tier, var(--ui-bg-accented)) 21%, var(--ui-bg-accented));
  border-color: var(--gear-rarity, var(--ui-text-muted));
}
</style>
