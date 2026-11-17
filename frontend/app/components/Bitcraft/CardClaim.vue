<script setup lang="ts">
import type { ClaimDescriptionState } from "~/types/ClaimDescriptionState";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";

export interface CardClaimProps {
  claim: ClaimDescriptionState;
  defaultMembers?: number;
}

import { tierToBackgroundClass, tierToTextClass } from "~/utils";

const showMoreMembers = ref(false);

const { claim, defaultMembers = 10 } = defineProps<CardClaimProps>();

const members = computed(() => {
  return claim.members.slice(0, showMoreMembers.value ? claim.members.length : defaultMembers);
});
const craftingStations = computed(() => (claim.stations ?? []).filter((station) => station.crafting));
const passiveStations = computed(() => (claim.stations ?? []).filter((station) => station.passive));
const stationCountTextClass = (tier: number) =>
  tier >= 10 ? "text-black" : tier === 7 ? "text-black" :
    [1, 2, 3, 4, 8].includes(tier) ? "text-white dark:text-black" : "text-white";

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
        <span class="text-gray-900 dark:text-gray-100"><BitcraftPlayerName :player-id="claim.owner_player_entity_id">{{ claimOwner }}</BitcraftPlayerName></span>
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
      <div class="py-2">
        <div class="mb-2 font-medium text-gray-500 dark:text-gray-400">Crafting stations</div>
        <div v-if="craftingStations.length" class="flex flex-wrap gap-1.5">
          <div v-for="station in craftingStations" :key="station.function_type" class="relative flex h-10 w-10 items-center justify-center rounded-md bg-gray-100 dark:bg-gray-800" :title="`${station.name} · Tier ${station.tier} · ${station.count} total`" :aria-label="`${station.name}, Tier ${station.tier}, ${station.count} total`" tabindex="0">
            <InventoryImg :item="station" :width="32" :height="32" />
            <span class="absolute -bottom-1 -right-1 rounded px-1 text-[10px] font-semibold leading-4" :class="[tierToBackgroundClass(station.tier), stationCountTextClass(station.tier)]">{{ station.count }}</span>
          </div>
        </div>
        <span v-else class="text-xs text-gray-400 dark:text-gray-500">None found</span>
      </div>
      <div class="py-2">
        <div class="mb-2 font-medium text-gray-500 dark:text-gray-400">Passive stations</div>
        <div v-if="passiveStations.length" class="flex flex-wrap gap-1.5">
          <div v-for="station in passiveStations" :key="station.function_type" class="relative flex h-10 w-10 items-center justify-center rounded-md bg-gray-100 dark:bg-gray-800" :title="`${station.name} · Tier ${station.tier} · ${station.count} total`" :aria-label="`${station.name}, Tier ${station.tier}, ${station.count} total`" tabindex="0">
            <InventoryImg :item="station" :width="32" :height="32" />
            <span class="absolute -bottom-1 -right-1 rounded px-1 text-[10px] font-semibold leading-4" :class="[tierToBackgroundClass(station.tier), stationCountTextClass(station.tier)]">{{ station.count }}</span>
          </div>
        </div>
        <span v-else class="text-xs text-gray-400 dark:text-gray-500">None found</span>
      </div>
      <div class="flex items-start justify-between gap-3 py-2">
        <span class="text-gray-500 dark:text-gray-400">Members ({{ claim.members.length }})</span>
        <span v-if="claim.members.length > 0" class="text-right text-gray-900 dark:text-gray-100">
          <template v-for="(member, index) of members" :key="index">
            <NuxtLink
              class="font-medium text-gray-900 hover:text-gray-700 dark:text-gray-100 dark:hover:text-gray-200"
              :to="{ name: 'players-id', params: { id: member.entity_id } }"
            >
              <BitcraftPlayerName :player-id="member.entity_id">{{ member.user_name }}</BitcraftPlayerName>
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
