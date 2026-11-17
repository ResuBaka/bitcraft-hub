<script setup lang="ts">
import type { SortingState } from "@tanstack/vue-table";
import { getSortedRowModel } from "@tanstack/vue-table";
import type { ClaimDescriptionStateMember } from "~/types/ClaimDescriptionStateMember";
import type { ItemExpended } from "~/types/ItemExpended";
import { expToLevel, levelToColor, levelToTier } from "~/utils";

const props = defineProps<{
  onlinePlayersCount: number;
  memberCount: number;
  memberSearch: string | null;
  showOnlyOnlineMembers: boolean;
  memberSorting: SortingState;
  memberColumns: any[];
  membersForTable: ({ permissions: number } & ClaimDescriptionStateMember)[];
  allMembers: ClaimDescriptionStateMember[];
  memberSkills: readonly string[];
  memberSecondarySkills: readonly string[];
  tierToColor: Record<number, string>;
  skillToToolIndex: Record<string, number | undefined>;
  tierToBgStyle: (tier: number) => { backgroundColor: string };
  getSkillTool: (member: ClaimDescriptionStateMember, skill: string) => ItemExpended | undefined;
  getToolLabel: (item: ItemExpended | undefined) => string | null;
}>();

const skillMaxLevels = computed(() => {
  const result: Record<string, { all: number | null; online: number | null }> = {};
  for (const skill of [...props.memberSkills, ...props.memberSecondarySkills]) {
    let all: number | null = null;
    let online: number | null = null;
    for (const member of props.allMembers) {
      const xp = member.skills_ranks?.[skill];
      if (xp == null) continue;
      const level = expToLevel(xp);
      all = Math.max(all ?? 0, level);
      if (member.online_state === "Online") online = Math.max(online ?? 0, level);
    }
    result[skill] = { all, online };
  }
  return result;
});

const showToolHints = ref(false);

const toolHint = (member: ClaimDescriptionStateMember, skill: string) => {
  const xp = member.skills_ranks?.[skill];
  if (props.skillToToolIndex[skill] === undefined || !member.inventory || xp == null) return null;

  const expectedTier = levelToTier(expToLevel(xp));
  const tool = props.getSkillTool(member, skill);
  if (!tool) return `No tool equipped; skill level supports tier ${expectedTier}.`;
  if (tool.tier && tool.tier < expectedTier) {
    return `Tool is tier ${tool.tier}; skill level supports tier ${expectedTier}.`;
  }
  return null;
};

const emit = defineEmits<{
  "update:memberSearch": [value: string | null];
  "update:showOnlyOnlineMembers": [value: boolean];
  "update:memberSorting": [value: SortingState];
  memberSearchChanged: [];
}>();
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex items-center gap-2">
        <UBadge :color="onlinePlayersCount > 0 ? 'success' : 'neutral'" variant="soft">
          {{ onlinePlayersCount }} online
        </UBadge>
        <span class="text-sm text-gray-500 dark:text-gray-400">{{ memberCount }} members</span>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <UInput
          :model-value="memberSearch"
          icon="i-heroicons-magnifying-glass"
          placeholder="Search members"
          class="w-full sm:w-64"
          @update:model-value="
            (value) => {
              emit('update:memberSearch', value);
              emit('memberSearchChanged');
            }
          "
        />
        <USwitch
          :model-value="showOnlyOnlineMembers"
          label="Only online"
          @update:model-value="(value) => emit('update:showOnlyOnlineMembers', value)"
        />
        <USwitch v-model="showToolHints" label="Tool hints" />
      </div>
    </div>
    <UTable
      :sorting="memberSorting"
      :columns="memberColumns"
      :data="membersForTable"
      :sorting-options="{ getSortedRowModel: getSortedRowModel() }"
      class="claim-table"
      @update:sorting="(value) => emit('update:memberSorting', value)"
    >
      <template #user-header="{ column }">
        <UButton
          variant="ghost"
          size="xs"
          class="-ml-2 font-semibold uppercase tracking-[0.08em]"
          @click="column.toggleSorting(column.getIsSorted() === 'asc')"
        >
          User
          <span class="ml-1 text-xs">{{
            column.getIsSorted() === "asc" ? "▲" : column.getIsSorted() === "desc" ? "▼" : ""
          }}</span>
        </UButton>
      </template>
      <template #permissions-header="{ column }">
        <UButton
          variant="ghost"
          size="xs"
          class="-ml-2 font-semibold uppercase tracking-[0.08em]"
          @click="column.toggleSorting(column.getIsSorted() === 'asc')"
        >
          Perms
          <span class="ml-1 text-xs">{{
            column.getIsSorted() === "asc" ? "▲" : column.getIsSorted() === "desc" ? "▼" : ""
          }}</span>
        </UButton>
      </template>
      <template
        v-for="skill in [...memberSkills, ...memberSecondarySkills]"
        :key="`${skill}-header`"
        #[`skill_${skill}-header`]="{ column }"
      >
        <div class="flex flex-col items-center text-center">
          <UButton
            variant="ghost"
            size="xs"
            class="px-1 font-semibold uppercase tracking-normal"
            @click="column.toggleSorting(column.getIsSorted() === 'asc')"
          >
            {{ skill }}
            <span class="ml-1 text-xs">{{
              column.getIsSorted() === "asc" ? "▲" : column.getIsSorted() === "desc" ? "▼" : ""
            }}</span>
          </UButton>
          <div class="text-xs normal-case tracking-normal text-gray-500 dark:text-gray-400">
            <div>Top: {{ skillMaxLevels[skill]?.all ?? "—" }}</div>
            <div>Online: {{ skillMaxLevels[skill]?.online ?? "—" }}</div>
          </div>
        </div>
      </template>
      <template #user-cell="{ row }">
        <NuxtLink
          :to="{ name: 'players-id', params: { id: row.original.entity_id } }"
          class="font-semibold hover:underline"
          :class="
            row.original.online_state === 'Online'
              ? 'text-emerald-600 dark:text-emerald-400'
              : 'text-gray-900 dark:text-gray-100'
          "
        >
          <BitcraftPlayerName :player-id="row.original.entity_id">{{ row.original.user_name }}</BitcraftPlayerName>
        </NuxtLink>
        <div class="text-xs text-gray-500 dark:text-gray-400">{{ row.original.online_state }}</div>
      </template>
      <template #permissions-cell="{ row }">
        <div class="flex items-center justify-center gap-1 text-lg">
          <span v-if="row.original.co_owner_permission">🏰</span>
          <span v-if="row.original.officer_permission">🗡️</span>
          <span v-if="row.original.build_permission">🔨</span>
          <span v-if="row.original.inventory_permission">📦</span>
        </div>
      </template>
      <template v-for="skill in memberSkills" :key="skill" #[`skill_${skill}-cell`]="{ row }">
        <div class="relative flex items-center justify-center">
          <span
            class="rounded-l-full px-2 py-1 text-sm font-bold"
            :class="[levelToColor(row.original?.skills_ranks?.[skill] ?? 0), showToolHints && toolHint(row.original, skill) ? 'border-r-2 border-amber-500' : 'border-r-0']"
            :style="tierToBgStyle(levelToTier(row.original?.skills_ranks?.[skill] ?? 0))"
            :title="showToolHints ? toolHint(row.original, skill) ?? undefined : undefined"
          >
            {{ expToLevel(row.original?.skills_ranks?.[skill] ?? 0) }}
          </span>
          <span
            v-if="getToolLabel(getSkillTool(row.original, skill))"
            class="rounded-r-full px-2 py-1 text-sm font-bold"
            :class="tierToColor[getSkillTool(row.original, skill)?.tier || 1]"
            :style="tierToBgStyle(getSkillTool(row.original, skill)?.tier || 1)"
          >
            {{ getToolLabel(getSkillTool(row.original, skill)) }}
          </span>
          <span v-else class="rounded-r-full px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
            >--</span
          >
        </div>
      </template>
      <template
        v-for="skill in memberSecondarySkills"
        :key="skill"
        #[`skill_${skill}-cell`]="{ row }"
      >
        <div class="relative flex items-center justify-center">
          <span
            class="rounded-l-full px-2 py-1 text-sm font-bold"
            :class="[levelToColor(row.original?.skills_ranks?.[skill] ?? 0), skillToToolIndex[skill] !== undefined ? '' : 'rounded-r-full', showToolHints && toolHint(row.original, skill) ? 'border-r-2 border-amber-500' : 'border-r-0']"
            :style="tierToBgStyle(levelToTier(row.original?.skills_ranks?.[skill] ?? 0))"
            :title="showToolHints ? toolHint(row.original, skill) ?? undefined : undefined"
          >
            {{ expToLevel(row.original?.skills_ranks?.[skill] ?? 0) }}
          </span>
          <span
            v-if="getToolLabel(getSkillTool(row.original, skill))"
            class="rounded-r-full px-2 py-1 text-sm font-bold"
            :class="tierToColor[getSkillTool(row.original, skill)?.tier || 1]"
            :style="tierToBgStyle(getSkillTool(row.original, skill)?.tier || 1)"
          >
            {{ getToolLabel(getSkillTool(row.original, skill)) }}
          </span>
          <span
            v-else-if="skillToToolIndex[skill] !== undefined"
            class="rounded-r-full px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
            >--</span
          >
        </div>
      </template>
    </UTable>
  </div>
</template>

<style scoped>
.claim-table :deep(thead tr th) {
  font-size: 0.75rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: rgba(100, 116, 139, 0.9);
}
.claim-table :deep(tbody tr td) {
  vertical-align: top;
}
.claim-table :deep(thead tr th:nth-child(n + 3)),
.claim-table :deep(tbody tr td:nth-child(n + 3)) {
  padding-inline: 0.25rem;
}
</style>
