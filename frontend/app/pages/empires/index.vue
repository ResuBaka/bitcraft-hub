<script setup lang="ts">
import type { EmpireSummary } from "~/types/EmpireSummary";

const { data, status, error, refresh } = await useFetchMsPack<EmpireSummary[]>("/api/bitcraft/empires");
const { search, searchTerm, page, filter } = useCatalogControls();
type EmpireSort = "members" | "name" | "territory";
const sort = filter<EmpireSort>("sort", v => v === "members" || v === "name" ? v : "territory", v => v === "territory" ? undefined : v);
const sortItems: { label: string; value: EmpireSort }[] = [
  { label: "Most territory", value: "territory" }, { label: "Most members", value: "members" }, { label: "Name A–Z", value: "name" },
];
const perPage = 24;
const filtered = computed(() => {
  const term = searchTerm.value?.toLowerCase() ?? "";
  const rows = (data.value ?? []).filter(e => `${e.name} ${e.capital_name ?? ""}`.toLowerCase().includes(term));
  if (sort.value !== "territory") rows.sort((a, b) =>
    (sort.value === "members" ? b.members - a.members : 0) || a.name.localeCompare(b.name, "en"));
  return rows;
});
const currentPage = computed({
  get: () => Math.min(page.value, Math.max(1, Math.ceil(filtered.value.length / perPage))),
  set: v => { page.value = v; },
});
const visible = computed(() => filtered.value.slice((currentPage.value - 1) * perPage, currentPage.value * perPage));
const totalChunks = computed(() => data.value?.reduce((n, e) => n + e.chunks, 0) ?? 0);
const fmt = (n: number) => n.toLocaleString("en-US");
useSeoMeta({ title: "Empires", description: "Explore BitCraft empires, their territory, citizens, and settlements." });
</script>

<template>
  <UContainer class="w-full max-w-none space-y-6 py-8">
    <header class="flex flex-wrap items-end justify-between gap-6 border-b border-default pb-6">
      <div>
        <p class="mb-1 text-xs font-semibold tracking-widest text-muted uppercase">World atlas</p>
        <h1 class="text-3xl font-bold tracking-tight">Empires</h1>
        <p class="mt-2 text-muted">Explore the territories, settlements, and people of BitCraft.</p>
      </div>
      <dl v-if="data" class="flex gap-6 tabular-nums">
        <div><dt class="text-xs text-muted">Empires</dt><dd class="text-xl font-semibold">{{ fmt(data.length) }}</dd></div>
        <div><dt class="text-xs text-muted">Controlled chunks</dt><dd class="text-xl font-semibold">{{ fmt(totalChunks) }}</dd></div>
      </dl>
    </header>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <UInput v-model="search" aria-label="Search empires or capitals" placeholder="Search empires or capitals" icon="i-heroicons-magnifying-glass" class="w-full sm:w-80" />
      <USelect v-model="sort" :items="sortItems" aria-label="Sort empires" class="w-44" />
    </div>
    <UProgress v-if="status === 'pending'" aria-label="Loading empires" />
    <div v-else-if="error" role="alert" class="space-y-3 border border-default p-5">
      <p>Could not load the empire directory.</p>
      <UButton label="Try again" variant="outline" @click="refresh()" />
    </div>
    <p v-else-if="!filtered.length" class="py-10 text-center text-muted">
      {{ searchTerm ? "No empires match your search." : "No empires have been synced yet." }}
    </p>
    <template v-else>
      <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
        <NuxtLink v-for="e in visible" :key="e.id" :to="`/empires/${e.id}`"
          class="group flex flex-col gap-4 rounded-lg border border-default bg-default p-4 transition hover:border-accented hover:bg-elevated focus-visible:outline-2">
          <div class="flex items-center gap-3">
            <BitcraftEmpireMark :empire="e" />
            <div class="min-w-0 flex-1">
              <h2 class="font-semibold break-words group-hover:underline">{{ e.name }}</h2>
              <p class="text-sm text-muted">{{ e.capital_name ?? (e.capital_id ? `Claim ${e.capital_id}` : "No known capital") }}</p>
            </div>
            <UBadge v-if="e.npc" label="NPC" color="neutral" variant="soft" />
            <UIcon name="i-heroicons-chevron-right" class="shrink-0 text-muted" />
          </div>
          <dl class="mt-auto grid grid-cols-3 gap-2 border-t border-default pt-3">
            <div><dt class="text-xs text-muted">Chunks</dt><dd class="font-semibold">{{ fmt(e.chunks) }}</dd></div>
            <div><dt class="text-xs text-muted">Members</dt><dd class="font-semibold">{{ fmt(e.members) }}</dd></div>
            <div><dt class="text-xs text-muted">Settlements</dt><dd class="font-semibold">{{ fmt(e.settlements) }}</dd></div>
          </dl>
        </NuxtLink>
      </div>
      <div class="flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-muted">{{ filtered.length }} matching {{ filtered.length === 1 ? "empire" : "empires" }}</p>
        <UPagination v-if="filtered.length > perPage" v-model:page="currentPage" :total="filtered.length" :items-per-page="perPage" :sibling-count="1" />
      </div>
    </template>
  </UContainer>
</template>
