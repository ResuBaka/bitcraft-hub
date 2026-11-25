<script setup lang="ts">
import type { BuffSnapshot } from "~/types/BuffSnapshot";
import { formatCountdown } from "~/utils/countdown";

const props = defineProps<{ buff: BuffSnapshot & { remaining: number | null } }>();
const name = computed(() => props.buff.name || `Buff #${props.buff.buff_id}`);
const open = ref(false);
const options = {
  signDisplay: "exceptZero",
  maximumFractionDigits: 2,
} as const;
const signedNumber = new Intl.NumberFormat(undefined, options);
const signedPercent = new Intl.NumberFormat(undefined, { ...options, style: "percent" });
</script>

<template>
  <UTooltip
    v-model:open="open"
    disable-closing-trigger
    :delay-duration="200"
    :ui="{ content: 'h-auto max-w-xs whitespace-normal p-0' }"
  >
    <button
      type="button"
      class="max-w-full cursor-help truncate rounded-sm text-left decoration-dotted underline-offset-2 hover:underline focus-visible:outline-2 focus-visible:outline-primary"
      @click="open = true"
    >
      <slot>{{ name }}</slot>
    </button>
    <template #content>
      <div class="space-y-1.5 p-3 text-xs">
        <p class="text-sm font-semibold">{{ name }}</p>
        <p v-if="buff.description">{{ buff.description }}</p>
        <dl v-if="buff.stats?.length" class="space-y-0.5">
          <div v-for="(stat, i) in buff.stats" :key="i" class="flex justify-between gap-4">
            <dt>{{ stat.name.replace(/([a-z0-9])([A-Z])/g, "$1 $2") }}</dt>
            <dd class="font-medium tabular-nums">
              {{ (stat.is_percent ? signedPercent : signedNumber).format(stat.value) }}
            </dd>
          </div>
        </dl>
        <p class="text-muted">
          <template v-if="buff.remaining == null">Permanent</template>
          <template v-else
            >{{ formatCountdown(buff.remaining * 1000) }} remaining<span v-if="buff.paused">
              (paused)</span
            ><span v-if="buff.duration > 0">
              · {{ formatCountdown(buff.duration * 1000) }} total</span
            ></template
          >
        </p>
      </div>
    </template>
  </UTooltip>
</template>
