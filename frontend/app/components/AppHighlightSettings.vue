<script setup lang="ts">
import type { ClaimResponse } from "~/types/ClaimResponse";
import type { PlayersResponse } from "~/types/PlayersResponse";
import type { HighlightedEntity } from "~/stores/config";
import { refDebounced } from "@vueuse/core";

const config = useConfigStore();
const playerSearch = ref("");
const claimSearch = ref("");
const playerQuery = refDebounced(playerSearch, 250);
const claimQuery = refDebounced(claimSearch, 250);
const selectedPlayer = ref<string | null>(null);
const selectedClaim = ref<string | null>(null);

const { data: players, pending: playersPending } = await useLazyFetchMsPack<PlayersResponse>(
  () => `/api/bitcraft/players?per_page=20&search=${encodeURIComponent(playerQuery.value)}`,
  { server: false },
);
const { data: claims, pending: claimsPending } = await useLazyFetchMsPack<ClaimResponse>(
  () => `/api/bitcraft/claims?per_page=20&search=${encodeURIComponent(claimQuery.value)}`,
  { server: false },
);

const playerOptions = computed(() => (players.value?.players ?? []).map((player) => ({
  value: String(player.entity_id), label: player.username,
})));
const claimOptions = computed(() => (claims.value?.claims ?? []).map((claim) => ({
  value: String(claim.entity_id), label: claim.name,
})));

function add(list: HighlightedEntity[], options: { value: string; label: string }[], id: string | null) {
  const option = options.find((item) => item.value === id);
  if (option && !list.some((item) => item.id === option.value)) {
    list.push({ id: option.value, name: option.label });
  }
}

watch(selectedPlayer, (id) => {
  if (!id) return;
  add(config.highlighted_players, playerOptions.value, id);
  selectedPlayer.value = null;
  playerSearch.value = "";
});
watch(selectedClaim, (id) => {
  if (!id) return;
  add(config.highlighted_claims, claimOptions.value, id);
  selectedClaim.value = null;
  claimSearch.value = "";
});
</script>

<template>
  <section class="space-y-4" aria-labelledby="settings-highlights">
    <div>
      <h2 id="settings-highlights" class="text-sm font-semibold text-highlighted">Highlights</h2>
      <p class="mt-1 text-xs text-muted">Highlighted players and claims are marked throughout the site on this browser.</p>
    </div>
    <UFormField label="Add player">
      <UInputMenu v-model="selectedPlayer" v-model:search-term="playerSearch" :items="playerOptions" value-key="value" label-key="label" :loading="playersPending" placeholder="Search players" class="w-full" />
    </UFormField>
    <ul v-if="config.highlighted_players.length" class="space-y-1">
      <li v-for="player in config.highlighted_players" :key="player.id" class="flex items-center justify-between gap-2 text-sm">
        <NuxtLink :to="`/players/${player.id}`"><BitcraftPlayerName :player-id="player.id">{{ player.name }}</BitcraftPlayerName></NuxtLink>
        <UButton icon="i-lucide-x" color="neutral" variant="ghost" size="xs" :aria-label="`Remove ${player.name} from highlights`" @click="config.highlighted_players = config.highlighted_players.filter((item) => item.id !== player.id)" />
      </li>
    </ul>
    <UFormField label="Add claim">
      <UInputMenu v-model="selectedClaim" v-model:search-term="claimSearch" :items="claimOptions" value-key="value" label-key="label" :loading="claimsPending" placeholder="Search claims" class="w-full" />
    </UFormField>
    <ul v-if="config.highlighted_claims.length" class="space-y-1">
      <li v-for="claim in config.highlighted_claims" :key="claim.id" class="flex items-center justify-between gap-2 text-sm">
        <NuxtLink :to="`/claims/${claim.id}`">{{ claim.name }}</NuxtLink>
        <UButton icon="i-lucide-x" color="neutral" variant="ghost" size="xs" :aria-label="`Remove ${claim.name} from highlights`" @click="config.highlighted_claims = config.highlighted_claims.filter((item) => item.id !== claim.id)" />
      </li>
    </ul>
  </section>
</template>
