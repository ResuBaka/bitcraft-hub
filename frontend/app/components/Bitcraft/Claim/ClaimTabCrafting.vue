<script setup lang="ts">
import { groupCraftRows } from "~/utils/passiveCraftGroups";
import PlayerName from "~/components/Bitcraft/PlayerName.vue";
import InventoryImg from "~/components/Bitcraft/InventoryImg.vue";
import { compareProjectOwners, compareStations } from "~/utils/stationSort";
import { watchDebounced } from "@vueuse/core";
import { registerWebsocketMessageHandler } from "~/composables/websocket";
import { isCraftVisible } from "~/utils/claimCraftFilter";
import { applyCraftProgress, applyPassiveCraftUpdate } from "~/utils/craftProgress";
import { remainingHexCoins, type HexCoinEstimate } from "~/utils/hexCoinEstimate";
import { passiveCraftTiming } from "~/utils/passiveCraftTiming";
type Craft = {
  id: string;
  building_id: string;
  building_name: string;
  building_icon_asset_name: string | null;
  building_function_types: number[];
  building_context: {
    claim_id: string | null;
    claim_name: string | null;
    location: { x: number | bigint; z: number | bigint; dimension: number | bigint; region: number } | null;
  } | null;
  owner_id: string;
  owner_name: string | null;
  is_member: boolean;
  recipe_id: number;
  recipe_name: string | null;
  outputs: { name: string; icon_asset_name: string; tier: number | null }[];
  kind: "Active" | "Passive";
  status: string;
  quantity: number;
  progress: number | null;
  progress_total: number | bigint | null;
  public: boolean | null;
  slot: number | null;
  timestamp: string | null;
  duration_seconds: number | null;
  hex_coin_estimate?: HexCoinEstimate | null;
};

const props = defineProps<{ claimId?: string; playerId?: string; claimXpRemainder?: number | null }>();
const playerView = computed(() => !!props.playerId);
const tierColors = useTierColor();
function tierColor(tier: number | null | undefined) {
  return tier == null ? undefined : tierColors[tier as keyof typeof tierColors];
}
const filter = ref("all");
const search = ref("");
const showAllActiveCrafts = ref(!!props.playerId);
const showFinishedActive = ref(!!props.playerId);
const showPassiveCrafts = ref(!!props.playerId);
const clock = ref(0);
let clockTimer: ReturnType<typeof setInterval> | undefined;
onMounted(() => {
  clock.value = Date.now();
  clockTimer = setInterval(() => { clock.value = Date.now(); }, 1000);
});
onBeforeUnmount(() => { if (clockTimer) clearInterval(clockTimer); });
const { data, pending, error, refresh } = useFetchMsPack<Craft[]>(
  () => props.playerId ? `/api/bitcraft/players/${props.playerId}/crafting` : `/api/bitcraft/claims/${props.claimId}/crafting`,
);
const websocket = useWebsocketStore();
const liveVersion = ref(0);
let refreshing = false;
let dirty = false;
const manualRefreshPending = ref(false);
async function refreshAll() {
  dirty = true;
  if (refreshing) return;
  refreshing = true;
  try {
    while (dirty) {
      dirty = false;
      await refresh();
    }
  } finally {
    refreshing = false;
  }
}
async function refreshManually() {
  manualRefreshPending.value = true;
  try {
    await refreshAll();
  } finally {
    manualRefreshPending.value = false;
  }
}
if (props.playerId) {
  registerWebsocketMessageHandler("PlayerCraftingChanged", () => ["player_crafting", `player_crafting.${props.playerId}`], (message) => {
    if (message.player_id === null || message.player_id === props.playerId) liveVersion.value++;
  });
} else {
  registerWebsocketMessageHandler("ClaimCraftingChanged", () => ["claim_crafting", `claim_crafting.${props.claimId}`], (message) => {
    if (message.claim_id === null || message.claim_id === props.claimId) liveVersion.value++;
  });
}
registerWebsocketMessageHandler("CraftProgressChanged", () => props.playerId ? [`player_craft_progress.${props.playerId}`] : [`claim_craft_progress.${props.claimId}`], (message) => {
  if (props.playerId ? message.player_id !== props.playerId : message.claim_id !== props.claimId) return;
  const updated = data.value && applyCraftProgress(data.value, message);
  if (updated) data.value = updated;
  // A project not present locally or an overlapping HTTP snapshot needs reconciliation.
  if (!updated || pending.value) liveVersion.value++;
});
registerWebsocketMessageHandler("PassiveCraftChanged", () => props.playerId ? [`player_passive_craft.${props.playerId}`] : [`claim_passive_craft.${props.claimId}`], (message) => {
  if (props.playerId ? message.player_id !== props.playerId : message.claim_id !== props.claimId) return;
  const updated = data.value && applyPassiveCraftUpdate(data.value, message);
  if (updated) data.value = updated;
  if (!updated || pending.value) liveVersion.value++;
});
watchDebounced(liveVersion, () => { void refreshAll(); }, { debounce: 400, maxWait: 2000 });
watch(() => websocket.isConnected, (connected) => { if (connected) liveVersion.value++; }, { immediate: true });
function onVisibilityChange() {
  if (document.visibilityState === "visible") void refreshAll();
}
onMounted(() => {
  document.addEventListener("visibilitychange", onVisibilityChange);
});
onBeforeUnmount(() => {
  document.removeEventListener("visibilitychange", onVisibilityChange);
});
// Locks can expire without a database update, so refresh when the next one expires.
let expiryTimer: ReturnType<typeof setTimeout> | undefined;
watch(data, (crafts) => {
  if (expiryTimer) clearTimeout(expiryTimer);
  if (!import.meta.client) return;
  const now = Date.now();
  const next = (crafts ?? [])
    .filter((craft) => craft.kind === "Active" && craft.status === "Active" && craft.timestamp)
    .map((craft) => new Date(craft.timestamp!).getTime())
    .filter((time) => Number.isFinite(time) && time > now)
    .reduce((earliest, time) => Math.min(earliest, time), Infinity);
  if (Number.isFinite(next)) {
    expiryTimer = setTimeout(() => { void refreshAll(); }, Math.min(next - now + 250, 2_147_483_647));
  }
}, { immediate: true });
onBeforeUnmount(() => { if (expiryTimer) clearTimeout(expiryTimer); });
const visible = computed(() => (data.value ?? []).filter((craft) => {
  if (!isCraftVisible(craft, showAllActiveCrafts.value, showFinishedActive.value, showPassiveCrafts.value)) return false;
  if (filter.value === "members" && !craft.is_member) return false;
  if (filter.value === "visitors" && craft.is_member) return false;
  const text = `${craft.building_name} ${craft.building_context?.claim_name ?? ""} ${craft.owner_name ?? craft.owner_id} ${craft.recipe_name ?? craft.recipe_id}`.toLowerCase();
  return text.includes(search.value.toLowerCase().trim());
}));
function groupBuildings(crafts: Craft[]) {
  const groups = new Map<string, { id: string; name: string; icon: string | null; context: Craft["building_context"]; functionTypes: number[]; crafts: Craft[] }>();
  for (const craft of crafts) {
    let group = groups.get(craft.building_id);
    if (!group) {
      group = { id: craft.building_id, name: craft.building_name, icon: craft.building_icon_asset_name, context: craft.building_context, functionTypes: craft.building_function_types ?? [], crafts: [] };
      groups.set(craft.building_id, group);
    }
    group.crafts.push(craft);
  }
  for (const group of groups.values()) {
    group.crafts.sort(compareProjectOwners);
  }
  return [...groups.values()].sort(compareStations);
}
const sections = computed(() => {
  return (["Active", "Passive"] as const).filter((kind) => kind === "Active" || showPassiveCrafts.value).map((kind) => ({
    kind,
    buildings: groupBuildings(visible.value.filter((craft) => craft.kind === kind)),
  }));
});
const expanded = ref<Set<string>>(new Set());
function toggleBuilding(kind: string, id: string) {
  const key = `${kind}-${id}`;
  const next = new Set(expanded.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expanded.value = next;
}

function detail(craft: Craft) {
  if (craft.kind === "Active") {
    if (craft.progress_total === null) return `Progress: ${craft.progress ?? 0} / unknown`;
    return `Progress: ${craft.progress ?? 0} / ${craft.progress_total}`;
  }
  return passiveCraftTiming([craft], clock.value);
}

function groupDetail(crafts: Craft[]) {
  const first = crafts[0]!;
  return first.kind === "Passive" ? passiveCraftTiming(crafts, clock.value) : detail(first);
}

function slots(crafts: Craft[]) {
  return crafts.flatMap((craft) => craft.slot === null ? [] : [craft.slot + 1]).sort((a, b) => a - b).join(", ");
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex flex-wrap items-center gap-3">
      <UInput v-model="search" placeholder="Search building, recipe or owner" icon="i-lucide-search" />
      <label v-if="!playerView" class="flex items-center gap-2 text-sm">
        Owner
        <select v-model="filter" class="rounded border border-default bg-default px-2 py-1.5">
          <option value="all">All</option>
          <option value="members">Claim members</option>
          <option value="visitors">Non-members</option>
        </select>
      </label>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="showAllActiveCrafts" type="checkbox" />
        Show all active crafts
      </label>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="showFinishedActive" type="checkbox" />
        Show finished active crafts
      </label>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="showPassiveCrafts" type="checkbox" />
        Show passive crafts
      </label>
      <UButton icon="i-lucide-refresh-cw" variant="outline" :loading="manualRefreshPending" @click="refreshManually">Refresh</UButton>
      <span class="text-xs text-muted">{{ websocket.isConnected ? 'Live updates connected' : 'Live updates unavailable; use Refresh or enable websockets' }}</span>
    </div>
    <p class="text-sm text-muted">{{ playerView ? 'Showing projects owned by this player, not projects they only contributed to.' : (showAllActiveCrafts ? 'Showing public and private active projects.' : 'Showing public active projects only.') }} Finished active crafts and passive stations can be shown using the checkboxes. Membership is based on the project owner’s current claim membership; it does not identify every contributor.</p>
    <p v-if="!playerView" class="text-sm text-muted">Estimated Hex Coins assume one progress per non-critical hit, unchanged claim technology and no other claim XP. The XP remainder is shared, so estimates for different projects cannot be added together.</p>
    <p v-if="error" class="text-error">Could not load crafting projects. Try refreshing.</p>
    <p v-else-if="pending && !data" class="text-muted">Loading crafting projects…</p>
    <template v-else>
    <section v-for="section in sections" :key="section.kind" class="flex flex-col gap-2">
      <h3 class="text-lg font-semibold">{{ section.kind }} stations</h3>
      <p v-if="section.buildings.length === 0" class="text-sm text-muted">No {{ section.kind.toLowerCase() }} crafting projects found.</p>
    <div v-else class="overflow-x-auto">
      <table class="w-full text-left text-sm">
        <thead><tr class="border-b border-default"><th class="p-2">Building</th><th v-if="playerView" class="p-2">Claim</th><th v-if="playerView" class="p-2">Location</th><th class="p-2">Projects</th><th class="p-2">Members</th><th class="p-2">Non-members</th><th class="p-2">Building page</th></tr></thead>
        <tbody v-for="building in section.buildings" :key="building.id">
          <tr class="border-b border-default">
            <td class="p-2">
              <button type="button" class="flex items-center gap-2 text-left font-medium hover:text-primary" :aria-expanded="expanded.has(`${section.kind}-${building.id}`)" @click="toggleBuilding(section.kind, building.id)">
                <UIcon :name="expanded.has(`${section.kind}-${building.id}`) ? 'i-lucide-chevron-down' : 'i-lucide-chevron-right'" />
                <InventoryImg :item="{ name: building.name, icon_asset_name: building.icon ?? '' }" :width="40" :height="40" class="shrink-0" />
                <span>{{ building.name }} <span class="text-xs text-muted">#{{ building.id }}</span></span>
              </button>
            </td>
            <td v-if="playerView" class="p-2">
              <NuxtLink v-if="building.context?.claim_id" :to="`/claims/${building.context.claim_id}`" class="text-primary hover:underline">{{ building.context.claim_name ?? `Claim #${building.context.claim_id}` }}</NuxtLink>
              <span v-else>{{ building.context ? 'Unclaimed' : 'Unknown' }}</span>
            </td>
            <td v-if="playerView" class="p-2">
              <template v-if="building.context?.location">
                N: {{ Math.ceil(Number(building.context.location.z) / 3) }}, E: {{ Math.ceil(Number(building.context.location.x) / 3) }}<br />
                <span class="text-xs text-muted">Region <BitcraftRegion :region="building.context.location.region" /> · Dimension {{ building.context.location.dimension }}</span>
              </template>
              <span v-else class="text-muted">Unknown</span>
            </td>
            <td class="p-2">{{ building.crafts.length }}</td>
            <td class="p-2">{{ building.crafts.filter((craft) => craft.is_member).length }}</td>
            <td class="p-2">{{ building.crafts.filter((craft) => !craft.is_member).length }}</td>
            <td class="p-2"><NuxtLink :to="`/buildings/${building.id}`" class="text-primary hover:underline">View building</NuxtLink></td>
          </tr>
          <tr v-if="expanded.has(`${section.kind}-${building.id}`)" class="border-b border-default">
            <td :colspan="playerView ? 7 : 5" class="p-2 sm:p-4">
              <div class="overflow-x-auto">
                <table class="w-full text-left text-sm">
                  <thead><tr class="border-b border-default"><th class="p-2">Recipe</th><th class="p-2">Owner</th><th class="p-2">Type / status</th><th class="p-2">Details</th><th v-if="!playerView" class="p-2">Est. Hex Coins</th></tr></thead>
                  <tbody>
                    <tr v-for="row in groupCraftRows(building.crafts)" :key="`${row[0]!.kind}-${row[0]!.id}`" class="border-b border-default align-top">
                      <td class="p-2"><div class="flex items-center gap-2"><div v-if="row[0]!.outputs?.length" class="flex shrink-0 flex-wrap gap-1"><InventoryImg v-for="(item, index) in row[0]!.outputs" :key="index" :item="item" :width="40" :height="40" :title="item.name" /></div><div><span :class="tierColor(row[0]!.outputs?.[0]?.tier)">{{ row[0]!.recipe_name ?? `Recipe #${row[0]!.recipe_id}` }}</span><div class="text-xs text-muted">{{ row[0]!.kind === 'Passive' ? `Projects: ${row.length}` : `Quantity: ${row[0]!.quantity}` }}</div></div></div></td>
                      <td class="p-2"><NuxtLink :to="`/players/${row[0]!.owner_id}`" class="text-primary hover:underline"><PlayerName :player-id="row[0]!.owner_id">{{ row[0]!.owner_name ?? `Player #${row[0]!.owner_id}` }}</PlayerName></NuxtLink><div><UBadge :color="row[0]!.is_member ? 'success' : 'warning'" variant="subtle">{{ row[0]!.is_member ? 'Member' : 'Non-member' }}</UBadge></div></td>
                      <td class="p-2">{{ row[0]!.kind }} · {{ row[0]!.status }}<div v-if="row[0]!.public !== null" class="text-xs text-muted">{{ row[0]!.public ? 'Public project' : 'Private project' }}</div><div v-if="slots(row)" class="text-xs text-muted">{{ row.length > 1 ? 'Slots' : 'Slot' }} {{ slots(row) }}</div></td>
                      <td class="p-2"><ClientOnly v-if="row[0]!.kind === 'Passive'"><span>{{ groupDetail(row) }}</span><template #fallback>—</template></ClientOnly><span v-else>{{ groupDetail(row) }}</span></td>
                      <td v-if="!playerView" class="p-2">{{ remainingHexCoins(row[0]!, claimXpRemainder)?.toLocaleString() ?? 'Unknown' }}<div v-if="row[0]!.kind === 'Passive'" class="text-xs text-muted">Passive crafts do not mint</div></td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    </section>
    </template>
  </div>
</template>
