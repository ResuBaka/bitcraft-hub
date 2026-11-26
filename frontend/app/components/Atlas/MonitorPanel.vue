<script setup lang="ts">
import { unpack } from "msgpackr/unpack";
import type { ClaimResponse } from "~/types/ClaimResponse";
import type { PlayersResponse } from "~/types/PlayersResponse";
import { MAX_RESOURCE_FILTERS } from "~/utils/atlasResources";
import {
  monitorEntityId,
  monitorKey,
  MAX_FAVORITES,
  MAX_TRACKED_TARGETS,
  type MonitorKind,
  type MonitorPreferences,
  type MonitorRosters,
  type MonitorTarget,
} from "~/utils/atlasMonitor";

const props = defineProps<{
  base?: string;
  preferences: MonitorPreferences;
  rosters: MonitorRosters;
  rosterLoading: boolean;
  rosterError: boolean;
  visiblePlayers: ReadonlySet<string>;
  streamStatus: string;
  resourceIds: readonly number[];
  showResources: boolean;
}>();
const emit = defineEmits<{
  toggle: [list: keyof MonitorPreferences, target: MonitorTarget];
  locate: [id: string];
  retry: [];
  resource: [id: number, enabled: boolean];
}>();

const kind = ref<MonitorKind>("player");
const query = ref("");
const results = shallowRef<MonitorTarget[]>([]);
const searchStatus = ref<"idle" | "loading" | "ready" | "error">("idle");
const groups = computed(() => [
  { key: "results", title: "Search results", targets: results.value },
  { key: "tracking", title: "Tracking", targets: props.preferences.tracking },
  { key: "favorites", title: "Favorites", targets: props.preferences.favorites },
]);
const keys = computed(() => ({
  tracking: new Set(props.preferences.tracking.map(monitorKey)),
  favorites: new Set(props.preferences.favorites.map(monitorKey)),
}));
function isTracked(target: MonitorTarget) {
  return keys.value.tracking.has(monitorKey(target));
}
function isFavorite(target: MonitorTarget) {
  return keys.value.favorites.has(monitorKey(target));
}
function isResourceShown(target: MonitorTarget) {
  return props.showResources && props.resourceIds.includes(Number(target.id));
}

watch([kind, query], ([selectedKind, input], _, cleanup) => {
  results.value = [];
  const search = input.trim();
  if (search.length < 2 || !props.base) {
    searchStatus.value = "idle";
    return;
  }
  searchStatus.value = "loading";
  const request = new AbortController();
  const timer = setTimeout(async () => {
    try {
      const bytes = await $fetch<ArrayBuffer>(
        `/api/bitcraft/${selectedKind === "player" ? "players" : "claims"}`,
        {
          baseURL: props.base,
          query: { search, page: 1, per_page: 12 },
          headers: { Accept: "application/vnd.msgpack" },
          responseType: "arrayBuffer",
          timeout: 5_000,
          retry: 0,
          signal: request.signal,
        },
      );
      if (request.signal.aborted) return;
      const data = unpack(new Uint8Array(bytes), { int64AsType: "auto" }) as
        | PlayersResponse
        | ClaimResponse;
      const rows = "players" in data ? data.players : data.claims;
      results.value = rows.flatMap((row) => {
        const id = String(row.entity_id);
        const name = "username" in row ? row.username : row.name;
        return monitorEntityId(id) && name ? [{ kind: selectedKind, id, name }] : [];
      });
      searchStatus.value = "ready";
    } catch {
      if (!request.signal.aborted) searchStatus.value = "error";
    }
  }, 250);
  cleanup(() => {
    clearTimeout(timer);
    request.abort();
  });
});

function visibleMembers(id: string) {
  return (props.rosters[id] ?? []).filter((member) => props.visiblePlayers.has(member.id));
}
</script>

<template>
  <aside class="monitor-panel" aria-label="Map monitor">
    <h2>Monitor</h2>
    <p class="monitor-note">
      Track live overworld positions in enabled regions. Favorites are saved in this browser.
    </p>
    <div class="monitor-search">
      <select v-model="kind" aria-label="Target type">
        <option value="player">Player</option>
        <option value="claim">Claim</option>
      </select>
      <input
        v-model="query"
        type="search"
        maxlength="80"
        :placeholder="`Search ${kind}s…`"
        :aria-label="`Search ${kind}s`"
      />
    </div>
    <p v-if="searchStatus === 'loading'" class="monitor-note" role="status">Searching…</p>
    <p v-else-if="searchStatus === 'error'" class="monitor-note" role="status">
      Search unavailable. Try again.
    </p>
    <p v-else-if="searchStatus === 'ready' && !results.length" class="monitor-note" role="status">
      No matches.
    </p>
    <p
      v-if="preferences.tracking.length && streamStatus !== 'live'"
      class="monitor-note"
      role="status"
    >
      {{
        streamStatus === "connecting"
          ? "Connecting to live players…"
          : streamStatus === "reconnecting"
            ? "Reconnecting to live players…"
            : "No live positions available."
      }}
    </p>
    <p v-if="rosterError" class="monitor-note" role="status">
      Claim roster refresh failed. <button type="button" @click="emit('retry')">Retry</button>
    </p>
    <section
      v-for="group in groups"
      v-show="group.key !== 'results' || group.targets.length"
      :key="group.key"
    >
      <h3>{{ group.title }}</h3>
      <p v-if="!group.targets.length" class="monitor-note">
        {{
          group.key === "tracking"
            ? "Track a player or claim to see it here."
            : "Save players, claims, or resources for quick access."
        }}
      </p>
      <ul v-else>
        <li v-for="target in group.targets" :key="monitorKey(target)">
          <div class="monitor-target">
            <span>{{ target.name }} <small>{{ target.kind }}</small></span>
            <button
              v-if="target.kind === 'resource'"
              type="button"
              :aria-label="`${isResourceShown(target) ? 'Hide' : 'Show'} ${target.name} on the map`"
              :disabled="
                !resourceIds.includes(Number(target.id)) &&
                resourceIds.length >= MAX_RESOURCE_FILTERS
              "
              :title="
                !resourceIds.includes(Number(target.id)) &&
                resourceIds.length >= MAX_RESOURCE_FILTERS
                  ? 'Clear a resource selection first'
                  : undefined
              "
              @click="emit('resource', Number(target.id), !isResourceShown(target))"
            >
              {{ isResourceShown(target) ? "Hide" : "Show" }}
            </button>
            <button
              v-else
              type="button"
              :aria-label="`${isTracked(target) ? 'Stop tracking' : 'Track'} ${target.name}`"
              :disabled="!isTracked(target) && preferences.tracking.length >= MAX_TRACKED_TARGETS"
              @click="emit('toggle', 'tracking', target)"
            >
              {{ isTracked(target) ? "Stop" : "Track" }}
            </button>
            <button
              type="button"
              :aria-label="`${isFavorite(target) ? 'Remove favorite:' : 'Favorite'} ${target.name}`"
              :aria-pressed="isFavorite(target)"
              :disabled="!isFavorite(target) && preferences.favorites.length >= MAX_FAVORITES"
              @click="emit('toggle', 'favorites', target)"
            >
              {{ isFavorite(target) ? "★" : "☆" }}
            </button>
          </div>
          <template v-if="group.key === 'tracking' && target.kind !== 'resource'">
            <div v-if="target.kind === 'player'" class="monitor-status">
              <button
                v-if="visiblePlayers.has(target.id)"
                type="button"
                @click="emit('locate', target.id)"
              >
                Locate on map
              </button>
              <span v-else>Not visible on the map</span>
            </div>
            <details v-else-if="rosters[target.id]" class="monitor-status">
              <summary>
                {{ visibleMembers(target.id).length }} visible ·
                {{ rosters[target.id]?.length }} members
              </summary>
              <ul v-if="visibleMembers(target.id).length" class="monitor-members">
                <li v-for="member in visibleMembers(target.id)" :key="member.id">
                  <button type="button" @click="emit('locate', member.id)">
                    {{ member.name || `Player ${member.id}` }} · Locate
                  </button>
                </li>
              </ul>
            </details>
            <p v-else class="monitor-note">
              {{ rosterLoading ? "Loading members…" : "Roster unavailable." }}
            </p>
          </template>
        </li>
      </ul>
    </section>
  </aside>
</template>

<style scoped>
.monitor-panel {
  display: grid;
  gap: 0.65rem;
  font-size: 0.83rem;
}
.monitor-panel h2 {
  font-weight: 700;
  font-size: 1rem;
}
.monitor-panel h3 {
  color: #f0c78c;
  font-weight: 600;
  margin-bottom: 0.3rem;
}
.monitor-note,
.monitor-status,
.monitor-target small {
  color: #a9c0c9;
  font-size: 0.75rem;
}
.monitor-search,
.monitor-target {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}
.monitor-search input,
.monitor-search select {
  min-width: 0;
  padding: 0.3rem;
  border: 1px solid #47616b;
  border-radius: 0.3rem;
  background: #102531;
}
.monitor-search input,
.monitor-target span {
  flex: 1;
}
.monitor-target span {
  overflow-wrap: anywhere;
}
.monitor-panel button {
  color: #f0c78c;
  cursor: pointer;
}
.monitor-target button {
  min-height: 1.8rem;
  min-width: 1.8rem;
}
.monitor-panel button:disabled {
  opacity: 0.45;
  cursor: default;
}
.monitor-status button,
.monitor-note button {
  text-decoration: underline;
}
.monitor-panel section > ul > li {
  padding: 0.2rem 0;
  border-top: 1px solid #324d59;
}
.monitor-members {
  padding-left: 0.75rem;
}
.monitor-status summary {
  cursor: pointer;
}
</style>
