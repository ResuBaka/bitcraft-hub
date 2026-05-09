<script setup lang="ts">
import type { ClaimDescriptionState } from "~/types/ClaimDescriptionState";

export interface CardClaimProps {
  claim: ClaimDescriptionState;
  defaultMembers?: number;
}

import { tierToTextClass } from "~/utils";

const showMoreMembers = ref(false);

const { claim, defaultMembers = 10 } = defineProps<CardClaimProps>();

const members = computed(() => {
  return claim.members.slice(0, showMoreMembers.value ? claim.members.length : defaultMembers);
});

const toggleShowMoreMembers = () => {
  showMoreMembers.value = !showMoreMembers.value;
};

const shouldShowMoreMembers = computed(() => {
  return claim.members.length > defaultMembers;
});

const claimOwner = computed(() => {
  if (claim === undefined) {
    return "";
  }

  return (
    claim.members.find((member) => member.entity_id === claim.owner_player_entity_id)?.user_name ??
    `No owner`
  );
});
</script>

<template>
  <UCard class="h-full" :ui="{ header: 'p-2 sm:p-4', body: 'p-2 sm:p-4' }">
    <template #header>
      <div class="flex items-start justify-between gap-3">
        <div>
          <NuxtLink
            class="text-base font-semibold hover:opacity-80"
            :class="tierToTextClass(claim.tier)"
            :to="{ name: 'claims-id', params: { id: claim.entity_id.toString() } }"
          >
            {{ claim.name }}
          </NuxtLink>
          <div class="mt-1 text-xs text-gray-500 dark:text-gray-400">
            Tier {{ claim.tier }} · Tiles {{ claim.num_tiles }}
            <template v-if="claim.location"
              >· Location R: <bitcraft-region :region="claim.region" /> N:
              {{ Math.ceil(claim.location.z / 3) }}, E:
              {{ Math.ceil(claim.location.x / 3) }}</template
            >
          </div>
        </div>
        <UButton
          v-if="shouldShowMoreMembers"
          variant="ghost"
          size="xs"
          icon="i-heroicons-chevron-down"
          :class="showMoreMembers ? 'rotate-180' : ''"
          @click="toggleShowMoreMembers"
        />
      </div>
    </template>

    <div class="divide-y divide-gray-100 dark:divide-gray-800 text-sm">
      <div class="flex items-start justify-between gap-3 py-2">
        <span class="text-gray-500 dark:text-gray-400">Owner</span>
        <span class="text-gray-900 dark:text-gray-100">{{ claimOwner }}</span>
      </div>
      <div class="flex items-start justify-between gap-3 py-2">
        <span class="text-gray-500 dark:text-gray-400">Supplies</span>
        <span class="text-gray-900 dark:text-gray-100">{{ parseInt(claim.supplies) }}</span>
      </div>
      <div v-if="claim.running_upgrade" class="flex items-start justify-between gap-3 py-2">
        <span class="text-gray-500 dark:text-gray-400">Research</span>
        <span class="text-gray-900 dark:text-gray-100">{{
          claim.running_upgrade.description
        }}</span>
      </div>
      <div class="flex items-start justify-between gap-3 py-2">
        <span class="text-gray-500 dark:text-gray-400">Members ({{ claim.members.length }})</span>
        <span v-if="claim.members.length > 0" class="text-right text-gray-900 dark:text-gray-100">
          <template v-for="(member, index) of members" :key="index">
            <NuxtLink
              class="font-medium text-gray-900 hover:text-gray-700 dark:text-gray-100 dark:hover:text-gray-200"
              :to="{ name: 'players-id', params: { id: member.entity_id } }"
            >
              {{ member.user_name }}
            </NuxtLink>
            {{ index + 1 < members.length ? ", " : "" }}
            {{
              shouldShowMoreMembers && index + 1 === defaultMembers && !showMoreMembers ? "..." : ""
            }}
          </template>
        </span>
        <span v-else class="text-gray-400 dark:text-gray-500">None</span>
      </div>
    </div>
  </UCard>
</template>
