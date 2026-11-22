<script setup lang="ts">
defineProps<{
  label?: string;
  item?: { name: string | null; rarity: string | null; tier: number | null } | null;
  stats?: { label: string; value: string }[];
}>();
</script>

<template>
  <UTooltip :delay-duration="250" :ui="{ content: 'h-auto max-w-xs whitespace-normal p-0' }">
    <template #content>
      <div class="space-y-2 p-3">
        <div>
          <p v-if="label" class="text-xs text-muted">{{ label }}</p>
          <p class="text-sm font-semibold">{{ item?.name || (item ? "Unknown item" : "Empty") }}</p>
        </div>
        <div v-if="item" class="flex flex-wrap gap-1.5">
          <UBadge color="neutral" variant="soft" size="sm">Rarity: {{ item.rarity ?? "—" }}</UBadge>
          <UBadge color="neutral" variant="soft" size="sm">Tier: {{ item.tier ?? "—" }}</UBadge>
        </div>
        <div v-if="item && (stats || $slots.details)" class="space-y-1 border-t border-default pt-2 text-xs">
          <template v-if="stats">
            <p class="font-medium text-muted">Stats</p>
            <dl v-if="stats.length" class="space-y-1">
              <div v-for="(stat, index) in stats" :key="index" class="flex justify-between gap-4">
                <dt>{{ stat.label }}</dt>
                <dd class="font-semibold tabular-nums">{{ stat.value }}</dd>
              </div>
            </dl>
            <p v-else class="text-muted">No stats available</p>
          </template>
          <slot name="details" />
        </div>
      </div>
    </template>
    <slot />
  </UTooltip>
</template>
