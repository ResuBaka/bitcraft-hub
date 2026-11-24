<script setup lang="ts">
import { overviewAlertHistoryKey } from "~/composables/useAccountOverviewAlerts";
import { overviewAlertTypes } from "~/utils/accountOverviewAlerts";

const history = inject(overviewAlertHistoryKey)!;
const { entries, clear } = history;
const dates = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "medium" });
</script>

<template>
  <UModal
    title="Alert history"
    description="The latest 200 alerts from the past 7 days in this browser."
  >
    <UButton label="Alert history" icon="i-lucide-history" color="neutral" variant="soft" />
    <template #body>
      <p v-if="!entries.length" class="text-sm text-muted">No recent alerts.</p>
      <ol v-else class="divide-y divide-default">
        <li v-for="entry in entries" :key="entry.id" class="py-2 text-sm">
          <div class="flex flex-wrap items-baseline justify-between gap-x-3">
            <strong>{{ entry.player.name }}</strong>
            <time :datetime="new Date(entry.time).toISOString()" class="text-xs text-muted">{{
              dates.format(entry.time)
            }}</time>
          </div>
          <p>{{ entry.message }}</p>
          <p
            v-if="entry.message !== overviewAlertTypes[entry.type].label"
            class="text-xs text-muted"
          >
            {{ overviewAlertTypes[entry.type].label }}
          </p>
        </li>
      </ol>
    </template>
    <template #footer>
      <UButton
        label="Clear history"
        variant="soft"
        color="neutral"
        :disabled="!entries.length"
        @click="clear"
      />
    </template>
  </UModal>
</template>
