<script setup lang="ts">
import type { WatchtowerDetail } from "~/types/WatchtowerDetail";
import { describeObservation } from "~/utils/watchtowerHistory";
import { empireLocation } from "~/utils/empireLocation";

definePageMeta({ key: route => String(route.params.id), validate: route => /^\d+$/.test(String(route.params.id)) });
const id = String(useRoute().params.id);
const cursors = ref<string[]>([]);
const query = computed(() => ({ before: cursors.value.at(-1) }));
const fetchedAt = useState<number | null>(`watchtower-reading:${id}`, () => null);
const { data, error, status, refresh } = await useFetchMsPack<WatchtowerDetail>(`/api/bitcraft/watchtowers/${id}`, {
  query, onResponse({ response }) { if (response.ok) fetchedAt.value = Date.now(); },
});
const missing = computed(() => error.value?.statusCode === 404);
if (import.meta.server && error.value) setResponseStatus(missing.value ? 404 : 502);
const tower = computed(() => data.value?.tower);
const history = computed(() => data.value?.history.map(row => ({ ...row, ...describeObservation(row) })) ?? []);
const name = computed(() => tower.value?.nickname || "Watchtower");
const reportLabels: Record<number, string> = {
  2: "Marked for siege", 3: "Attack started", 4: "Defense started", 5: "Siege won",
  6: "Defense succeeded", 7: "Siege failed", 8: "Defense lost", 10: "Watchtower built",
};
useSeoMeta({ title: () => `${name.value} · Watchtowers` });
</script>

<template>
  <UContainer class="w-full max-w-none space-y-6 py-8">
    <nav class="flex gap-2 text-sm text-muted" aria-label="Breadcrumb">
      <NuxtLink to="/empires" class="hover:underline">Empires</NuxtLink>
      <template v-if="tower?.empire_name"> / <NuxtLink :to="`/empires/${tower.empire_id}#watchtowers`" class="hover:underline">{{ tower.empire_name }}</NuxtLink></template>
    </nav>
    <section v-if="error" role="alert">
      <h1 class="text-xl font-semibold">{{ missing ? "Watchtower not found" : "Could not load this watchtower" }}</h1>
      <UButton v-if="!missing" class="mt-3" label="Try again" variant="outline" @click="refresh()" />
    </section>
    <template v-else-if="data">
      <header class="space-y-2 border-b border-default pb-4">
        <p class="text-xs font-semibold tracking-widest text-muted uppercase">Watchtower record</p>
        <h1 class="text-3xl font-bold break-words">{{ name }}</h1>
        <p class="text-sm text-muted">Tower {{ id }}<template v-if="tower"> · <template v-if="tower.region != null">Region {{ tower.region }} · </template>{{ empireLocation(tower.x, tower.z) }} · Dimension {{ tower.dimension }}</template></p>
        <UBadge :label="tower ? (tower.active ? 'Active' : 'Inactive') : 'No longer present'" :color="tower?.active ? 'success' : 'neutral'" variant="soft" />
      </header>
      <dl v-if="tower" class="grid grid-cols-2 gap-4 rounded-lg border border-default p-4 lg:grid-cols-5">
        <div><dt class="text-sm text-muted">Energy</dt><dd class="text-lg font-semibold">{{ tower.energy.toLocaleString('en-US') }}</dd></div>
        <div><dt class="text-sm text-muted">Upkeep per tick</dt><dd class="text-lg font-semibold">{{ tower.upkeep }}</dd></div>
        <div><dt class="text-sm text-muted">Empty by</dt><dd class="text-lg font-semibold"><BitcraftWatchtowerDeadline :tower="tower" :fetched-at="fetchedAt" /></dd></div>
        <div><dt class="text-sm text-muted">Controlled chunks</dt><dd class="text-lg font-semibold">{{ tower.controlled_chunks }} / {{ tower.assigned_chunks }}</dd></div>
        <div><dt class="text-sm text-muted">Siege status</dt><dd class="text-lg font-semibold">{{ tower.under_siege ? "Under siege" : tower.marked ? "Marked for siege" : "Not under siege" }}</dd></div>
      </dl>
      <p v-if="tower?.active && !tower.under_siege && tower.energy > 0 && tower.upkeep > 0" class="text-sm text-muted">
        Assumes no refills or changes to upkeep. Energy drops on each upkeep tick; the next tick may come sooner than a full interval.
      </p>
      <section v-if="data.sieges.length">
        <h2 class="text-xl font-semibold">Siege participants</h2>
        <ul class="mt-3 divide-y divide-default rounded-lg border border-default px-4">
          <li v-for="s in data.sieges" :key="s.id" class="flex flex-wrap justify-between gap-2 py-3 text-sm">
            <div>
              <NuxtLink :to="`/empires/${s.empire_id}`" class="font-semibold hover:underline">{{ s.empire_name || `Empire ${s.empire_id}` }}</NuxtLink>
              · {{ !s.active ? "Marked for siege" : s.empire_id === tower?.empire_id ? "Defending" : "Attacking" }}
              <p v-if="s.start_timestamp" class="text-muted">Started <LocalDateTime :datetime="s.start_timestamp" seconds /></p>
            </div>
            <span>Energy {{ s.energy.toLocaleString('en-US') }}</span>
          </li>
        </ul>
      </section>
      <section class="space-y-3" aria-labelledby="history-title">
        <div class="flex items-center justify-between gap-3">
          <h2 id="history-title" class="text-xl font-semibold">Observation history</h2>
          <UButton label="Refresh" icon="i-heroicons-arrow-path" variant="outline" :loading="status === 'pending'" @click="refresh()" />
        </div>
        <p class="text-sm text-muted">Newest first. Times show when the tracker observed a change. Changes made while offline may appear together after reconnection.</p>
        <ol v-if="history.length" class="divide-y divide-default border-y border-default" :aria-busy="status === 'pending'">
          <li v-for="row in history" :key="row.id" class="flex flex-wrap justify-between gap-2 py-3">
            <div><h3 class="font-semibold">{{ row.title }}</h3><p class="text-sm text-muted">{{ row.details.join(" · ") }}</p>
              <p v-if="row.current?.start_timestamp && row.previous?.start_timestamp !== row.current.start_timestamp" class="text-sm text-muted">
                Siege start: <LocalDateTime :datetime="row.current.start_timestamp" seconds />
              </p>
            </div>
            <LocalDateTime :datetime="row.observed_at" seconds class="text-xs text-muted" />
          </li>
        </ol>
        <p v-else class="text-sm text-muted">No observations on this page.</p>
        <div v-if="cursors.length || data.next_before" class="flex gap-2">
          <UButton label="Newer" variant="outline" :disabled="!cursors.length || status === 'pending'" @click="cursors.pop()" />
          <UButton label="Older" variant="outline" :disabled="!data.next_before || status === 'pending'" @click="data.next_before && cursors.push(data.next_before)" />
        </div>
      </section>
      <section v-if="data.reports.length" class="space-y-3">
        <h2 class="text-xl font-semibold">Game reports at this location</h2>
        <p class="text-sm text-muted">The latest game notifications for this map tile, from each reporting empire’s perspective. Older reports can refer to a previous tower on the same tile.</p>
        <ol class="divide-y divide-default border-y border-default">
          <li v-for="r in data.reports" :key="r.id" class="flex flex-wrap items-baseline justify-between gap-2 py-3">
            <p><strong>{{ reportLabels[r.notification_type] }}</strong> ·
              <NuxtLink :to="`/empires/${r.empire_id}`" class="text-sm hover:underline">{{ r.empire_name || `Empire ${r.empire_id}` }}</NuxtLink>
            </p>
            <LocalDateTime class="text-xs text-muted" :datetime="r.timestamp * 1000" seconds />
          </li>
        </ol>
      </section>
    </template>
    <UProgress v-else aria-label="Loading watchtower" />
  </UContainer>
</template>
