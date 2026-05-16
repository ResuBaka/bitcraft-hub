<script setup lang="ts">
import type { ClaimTechDesc } from "~/types/ClaimTechDesc";

defineProps<{
  claimTier: number;
  unlockedUpgradesCount: number;
  upgradesSorted: ClaimTechDesc[];
  upgradesByTier: [number, ClaimTechDesc[]][];
  isUpgradeLocked: (upgrade: ClaimTechDesc) => boolean;
  isUpgradeLearned: (upgrade: ClaimTechDesc) => boolean;
  unlocksTechNames: (upgrade: ClaimTechDesc) => string[];
}>();
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="grid gap-3 md:grid-cols-3">
      <div class="rounded-lg border border-gray-200 p-3 dark:border-gray-800">
        <div class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
          Claim tier
        </div>
        <div class="text-lg font-semibold text-gray-900 dark:text-gray-100">T{{ claimTier }}</div>
      </div>
      <div class="rounded-lg border border-gray-200 p-3 dark:border-gray-800">
        <div class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">
          Unlocked upgrades
        </div>
        <div class="text-lg font-semibold text-gray-900 dark:text-gray-100">
          {{ unlockedUpgradesCount }} / {{ upgradesSorted.length || 0 }}
        </div>
      </div>
    </div>
    <div v-for="[tier, upgrades] in upgradesByTier" :key="tier" class="space-y-3">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="text-base font-semibold text-gray-900 dark:text-gray-100">Tier {{ tier }}</h3>
        <UBadge variant="soft" color="neutral">{{ upgrades.length }} upgrades</UBadge>
      </div>
      <div class="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        <div
          v-for="upgrade in upgrades"
          :key="upgrade.id"
          class="rounded-lg border border-gray-200 p-4 shadow-sm transition dark:border-gray-800"
          :class="isUpgradeLocked(upgrade) ? 'opacity-70' : ''"
        >
          <div class="flex items-start justify-between gap-2">
            <div>
              <div class="text-sm font-semibold text-gray-900 dark:text-gray-100">
                {{ upgrade.name }}
              </div>
              <div class="text-xs text-gray-500 dark:text-gray-400">{{ upgrade.description }}</div>
            </div>
            <UBadge variant="soft" color="neutral">T{{ upgrade.tier }}</UBadge>
          </div>
          <div class="mt-2 text-xs text-gray-500 dark:text-gray-400">
            Type: {{ upgrade.tech_type }}
          </div>
          <div
            v-if="upgrade.unlocks_techs?.length"
            class="mt-1 text-xs text-gray-500 dark:text-gray-400"
          >
            Unlocks: {{ unlocksTechNames(upgrade).join(", ") }}
          </div>
          <div class="mt-3">
            <UBadge v-if="isUpgradeLocked(upgrade)" variant="soft" color="neutral"
              >Requires Claim Tier {{ upgrade.tier }}</UBadge
            >
            <UBadge v-else-if="isUpgradeLearned(upgrade)" variant="soft" color="primary"
              >Learned</UBadge
            >
            <UBadge v-else variant="soft" color="success">Available</UBadge>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
