<script setup lang="ts">
import type { EmpireDetail } from "~/types/EmpireDetail";
import type { EmpireNews } from "~/types/EmpireNews";
import { empireLocation, formatEmpireNewsArgument } from "~/utils/empireLocation";

definePageMeta({
  key: route => String(route.params.id),
  validate: route => /^\d+$/.test(String(route.params.id)),
});
const id = String(useRoute().params.id);
const fetchedAt = useState<number | null>(`empire-reading:${id}`, () => null);
const { data, error, status, refresh } = await useFetchMsPack<EmpireDetail>(`/api/bitcraft/empires/${id}`, {
  onResponse({ response }) { if (response.ok) fetchedAt.value = Date.now(); },
});
const missing = computed(() => error.value?.statusCode === 404);
if (import.meta.server && error.value) setResponseStatus(missing.value ? 404 : 502);

const memberSearch = ref("");
const memberPage = ref(1);
const memberSize = 15;
const members = computed(() => {
  const query = memberSearch.value.trim().toLowerCase();
  return (data.value?.members ?? []).filter(m => `${m.username ?? m.id} ${m.title ?? ""}`.toLowerCase().includes(query));
});
watch(memberSearch, () => { memberPage.value = 1; });
const roster = computed(() => members.value.slice((memberPage.value - 1) * memberSize, memberPage.value * memberSize));
const fmt = (n: number) => n.toLocaleString("en-US");
function newsText(n: EmpireNews) {
  const args = n.replacements.map(formatEmpireNewsArgument);
  return n.template?.replace(/\{(\d+)\}/g, (token, i) => args[Number(i)] ?? token)
    ?? (args.join(" · ") || "Empire activity");
}
useSeoMeta({
  title: () => data.value ? `${data.value.empire.name} · Empires` : "Empire",
  description: () => `Territory, settlements, members, and recent activity for ${data.value?.empire.name ?? "this BitCraft empire"}.`,
});
</script>

<template>
  <UContainer class="w-full max-w-none space-y-6 py-8">
    <NuxtLink to="/empires" class="inline-flex items-center gap-1 text-sm text-muted hover:underline">
      <UIcon name="i-heroicons-arrow-left" /> All empires
    </NuxtLink>
    <UProgress v-if="status === 'pending'" aria-label="Loading empire" />
    <section v-else-if="error" class="space-y-2" role="alert">
      <h1 class="text-2xl font-semibold">{{ missing ? "Empire not found" : "Could not load this empire" }}</h1>
      <p class="text-muted">{{ missing ? "This empire is not in the current directory." : "The empire data is temporarily unavailable." }}</p>
      <UButton v-if="!missing" label="Try again" variant="outline" @click="refresh()" />
    </section>
    <template v-else-if="data">
      <header class="flex items-center gap-5 border-b border-default pb-6">
        <BitcraftEmpireMark :empire="data.empire" class="text-2xl" />
        <div class="min-w-0">
          <p class="text-xs font-semibold tracking-widest text-muted uppercase">{{ data.empire.npc ? "NPC empire" : "Player empire" }}</p>
          <h1 class="mt-1 text-3xl font-bold tracking-tight break-words">{{ data.empire.name }}</h1>
          <p v-if="data.empire.capital_id" class="mt-1 text-sm text-muted">Capital:
            <NuxtLink :to="`/claims/${data.empire.capital_id}`" class="text-default underline">{{ data.empire.capital_name ?? `Claim ${data.empire.capital_id}` }}</NuxtLink>
          </p>
        </div>
      </header>
      <dl class="grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-default bg-accented md:grid-cols-4">
        <div v-for="[label, value] in [
          ['Controlled chunks', fmt(data.empire.chunks)], ['Members', fmt(data.empire.members)],
          ['Settlements', fmt(data.empire.settlements)], ['Active watchtowers', `${fmt(data.resources.active_towers)} / ${fmt(data.empire.towers)}`],
        ]" :key="label" class="bg-default px-4 py-3">
          <dt class="text-xs text-muted">{{ label }}</dt><dd class="mt-1 text-xl font-semibold tabular-nums">{{ value }}</dd>
        </div>
      </dl>
      <section v-if="data.resources.directive?.trim()" class="rounded-lg border-l-4 border-accented bg-elevated px-5 py-3">
        <h2 class="text-xs font-semibold tracking-widest text-muted uppercase">Empire directive</h2>
        <p class="mt-1 break-words whitespace-pre-line">{{ data.resources.directive }}</p>
      </section>
      <BitcraftEmpireWatchtowers v-if="data.towers" :towers="data.towers" :former="data.former_towers" :fetched-at="fetchedAt" />
      <div class="grid items-start gap-6 lg:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <section class="min-w-0 space-y-4">
          <div class="flex flex-wrap items-center justify-between gap-3">
            <h2 class="text-xl font-semibold">Member roster</h2>
            <UInput v-if="data.members.length" v-model="memberSearch" aria-label="Filter members" placeholder="Filter by name or rank" icon="i-heroicons-magnifying-glass" />
          </div>
          <div v-if="members.length" class="overflow-x-auto rounded-lg border border-default">
            <table class="w-full text-left text-sm">
              <thead class="bg-elevated text-muted"><tr>
                <th scope="col" class="px-3 py-2">Player</th><th scope="col" class="px-3 py-2">Rank</th>
                <th scope="col" class="px-3 py-2 text-right">Shards donated</th><th scope="col" class="px-3 py-2 text-right">Currency donated</th>
              </tr></thead>
              <tbody class="divide-y divide-default">
                <tr v-for="m in roster" :key="m.id">
                  <td class="px-3 py-2"><NuxtLink :to="`/players/${m.id}`" class="font-medium hover:underline">{{ m.username ?? `Player ${m.id}` }}</NuxtLink></td>
                  <td class="px-3 py-2">{{ m.title || `Rank ${m.rank}` }}</td>
                  <td class="px-3 py-2 text-right tabular-nums">{{ fmt(m.donated_shards) }}</td>
                  <td class="px-3 py-2 text-right tabular-nums">{{ fmt(m.donated_currency) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <p v-else class="text-sm text-muted">{{ data.members.length ? "No matching members." : "No recorded members." }}</p>
          <UPagination v-if="members.length > memberSize" v-model:page="memberPage" :total="members.length" :items-per-page="memberSize" :sibling-count="1" />
        </section>
        <aside class="space-y-5">
          <section class="rounded-lg border border-default p-4">
            <h2 class="text-lg font-semibold">Settlements</h2>
            <ul v-if="data.settlements.length" class="mt-2 divide-y divide-default">
              <li v-for="s in data.settlements" :key="s.id" class="flex items-start justify-between gap-2 py-2">
                <div>
                  <NuxtLink :to="`/claims/${s.id}`" class="font-medium hover:underline">{{ s.name ?? `Claim ${s.id}` }}</NuxtLink>
                  <p class="text-xs text-muted"><template v-if="s.region != null">Region {{ s.region }} · </template>{{ empireLocation(s.x, s.z) }}</p>
                </div>
                <UBadge v-if="s.id === data.empire.capital_id" label="Capital" color="neutral" variant="soft" />
              </li>
            </ul>
            <p v-else class="mt-2 text-sm text-muted">No affiliated settlements.</p>
          </section>
          <section class="rounded-lg border border-default p-4">
            <h2 class="text-lg font-semibold">Empire resources</h2>
            <dl class="mt-3 grid grid-cols-2 gap-3 text-sm">
              <div v-for="[label, value] in [
                ['Shard treasury', data.resources.shard_treasury], ['Currency treasury', data.resources.currency_treasury],
                ['Foundries', data.resources.foundries], ['Active sieges', data.resources.active_sieges],
              ]" :key="label"><dt class="text-muted">{{ label }}</dt><dd class="font-semibold tabular-nums">{{ value.toLocaleString('en-US') }}</dd></div>
            </dl>
          </section>
          <section class="rounded-lg border border-default p-4">
            <h2 class="text-lg font-semibold">Recent activity</h2>
            <ol v-if="data.news.length" class="mt-2 divide-y divide-default">
              <li v-for="n in data.news" :key="n.id" class="py-2">
                <p class="text-sm break-words">{{ newsText(n) }}</p>
                <LocalDateTime :datetime="n.timestamp * 1000" class="text-xs text-muted" />
              </li>
            </ol>
            <p v-else class="mt-2 text-sm text-muted">No recent empire activity.</p>
          </section>
        </aside>
      </div>
    </template>
  </UContainer>
</template>
