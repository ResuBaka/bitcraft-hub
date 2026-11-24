<script setup lang="ts">
import type { Watchtower } from "~/types/Watchtower";
import type { FormerWatchtower } from "~/types/FormerWatchtower";
import { empireLocation } from "~/utils/empireLocation";

const props = defineProps<{ towers: Watchtower[]; former: FormerWatchtower[]; fetchedAt: number | null }>();
const page = ref(1);
const size = 6;
const visible = computed(() => props.towers.slice((page.value - 1) * size, page.value * size));
</script>

<template>
  <section id="watchtowers" class="space-y-3 border-b border-default pb-6">
    <h2 class="text-xl font-semibold">Watchtowers <span class="text-sm text-muted">({{ towers.length }})</span></h2>
    <p v-if="!towers.length" class="text-sm text-muted">This empire currently holds no watchtowers.</p>
    <div v-else class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
      <NuxtLink v-for="t in visible" :key="t.id" :to="`/watchtowers/${t.id}`"
        class="group space-y-3 rounded-lg border border-default p-4 hover:bg-elevated focus-visible:outline-2">
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div>
            <h3 class="font-semibold group-hover:underline">{{ t.nickname || "Watchtower" }}</h3>
            <p class="text-xs text-muted"><template v-if="t.region != null">Region {{ t.region }} · </template>{{ empireLocation(t.x, t.z) }}</p>
          </div>
          <UBadge :label="t.active ? 'Active' : 'Inactive'" :color="t.active ? 'success' : 'neutral'" variant="soft" />
        </div>
        <div class="flex flex-wrap gap-x-4 gap-y-1 text-sm">
          <span>Energy <b>{{ t.energy.toLocaleString('en-US') }}</b></span>
          <span>Upkeep <b>{{ t.upkeep }}</b></span>
          <span>Chunks <b>{{ t.controlled_chunks }} / {{ t.assigned_chunks }}</b></span>
        </div>
        <p class="text-sm">Empty by: <b><BitcraftWatchtowerDeadline :tower="t" :fetched-at="fetchedAt" /></b></p>
        <UBadge v-if="t.under_siege || t.marked" :label="t.under_siege ? 'Under siege' : 'Marked for siege'" color="warning" variant="soft" />
      </NuxtLink>
    </div>
    <UPagination v-if="towers.length > size" v-model:page="page" :total="towers.length" :items-per-page="size" :sibling-count="1" />
    <details v-if="former.length" class="text-sm">
      <summary class="cursor-pointer font-semibold">Former watchtowers ({{ former.length }})</summary>
      <ul class="mt-2 space-y-1">
        <li v-for="t in former" :key="t.id"><NuxtLink :to="`/watchtowers/${t.id}`" class="underline">
          {{ t.nickname || "Watchtower" }} · {{ empireLocation(t.x, t.z) }}
        </NuxtLink></li>
      </ul>
    </details>
  </section>
</template>
