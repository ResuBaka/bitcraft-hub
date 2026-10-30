<script setup lang="ts">
import type { TableColumn, TabsItem } from "@nuxt/ui";
import type { ClaimEvent } from "~/types/ClaimEvent";
import type { ClaimEventsResponse } from "~/types/ClaimEventsResponse";
import type { ClaimEventType } from "~/types/ClaimEventType";
import type { ClaimMemberPermissions } from "~/types/ClaimMemberPermissions";

const props = defineProps<{
  claimId: string;
  region: number;
  members: Record<string, { user_name: string }>;
}>();

const groups = {
  all: {
    label: "All events",
    icon: "i-lucide-list",
    description: "All recorded claim activity, newest first.",
  },
  treasury: {
    label: "Treasury",
    icon: "i-lucide-coins",
    description:
      "Deposits and withdrawals, with the treasury balance after each transaction. XP-generated coins are excluded.",
  },
  research: {
    label: "Research",
    icon: "i-lucide-flask-conical",
    description: "Completed research and claim technology milestones.",
  },
  buildings: {
    label: "Buildings",
    icon: "i-lucide-hammer",
    description: "Buildings placed in or removed from the claim.",
  },
  members: {
    label: "Members",
    icon: "i-lucide-users",
    description: "Members joining, leaving, and changes to their permissions.",
  },
} as const;
type EventGroup = keyof typeof groups;
const tabs: TabsItem[] = Object.entries(groups).map(([value, group]) => ({
  value,
  label: group.label,
  icon: group.icon,
}));
const activeGroup = ref<EventGroup>("all");
const page = ref(1);
const perPage = 20;

watch(
  () => [props.claimId, props.region, activeGroup.value],
  () => {
    page.value = 1;
  },
  { flush: "sync" },
);

const { data, pending, error, refresh } = useFetchMsPack<ClaimEventsResponse>(() => {
  const group = activeGroup.value === "all" ? "" : `&group=${activeGroup.value}`;
  return `/claims/${props.claimId}/events?region=${props.region}&page=${page.value}&per_page=${perPage}${group}`;
});

const eventStyles = {
  TreasuryDeposit: {
    label: "Deposit",
    icon: "i-lucide-arrow-down-left",
    color: "success",
    group: "treasury",
  },
  TreasuryWithdrawal: {
    label: "Withdrawal",
    icon: "i-lucide-arrow-up-right",
    color: "error",
    group: "treasury",
  },
  ResearchCompleted: {
    label: "Research completed",
    icon: "i-lucide-flask-conical",
    color: "success",
    group: "research",
  },
  BuildingPlaced: {
    label: "Building placed",
    icon: "i-lucide-house-plus",
    color: "info",
    group: "buildings",
  },
  BuildingRemoved: {
    label: "Building removed",
    icon: "i-lucide-house-minus",
    color: "warning",
    group: "buildings",
  },
  MemberAdded: {
    label: "Member joined",
    icon: "i-lucide-user-plus",
    color: "success",
    group: "members",
  },
  MemberRemoved: {
    label: "Member left",
    icon: "i-lucide-user-minus",
    color: "warning",
    group: "members",
  },
  MemberPermissionsChanged: {
    label: "Permissions changed",
    icon: "i-lucide-shield-check",
    color: "info",
    group: "members",
  },
} as const satisfies Record<
  ClaimEventType,
  { label: string; icon: string; color: string; group: EventGroup }
>;

const permissionLabels: Record<keyof ClaimMemberPermissions, string> = {
  inventory_permission: "Inventory",
  build_permission: "Build",
  officer_permission: "Officer",
  co_owner_permission: "Co-owner",
};

function permissionChanges(event: ClaimEvent) {
  const before = event.permissions_before;
  const after = event.permissions_after;
  if (!before || !after) return [];
  return (Object.keys(permissionLabels) as (keyof ClaimMemberPermissions)[])
    .filter((key) => before[key] !== after[key])
    .map((key) => ({ label: permissionLabels[key], granted: after[key] }));
}

function playerName(id: bigint) {
  return props.members[String(id)]?.user_name ?? `Player #${id}`;
}

function subject(event: ClaimEvent) {
  if (event.subject_name) return event.subject_name;
  if (event.member_entity_id !== null) return playerName(event.member_entity_id);
  if (event.research_id !== null) return `Research #${event.research_id}`;
  if (event.building_description_id !== null) return `Building #${event.building_description_id}`;
  return eventStyles[event.event_type].label;
}

function formatAmount(value: bigint | null) {
  return value === null ? "—" : value.toLocaleString();
}

function signedAmount(event: ClaimEvent) {
  if (event.amount === null) return "—";
  return `${event.event_type === "TreasuryWithdrawal" ? "−" : "+"}${formatAmount(event.amount)}`;
}

const treasuryColumns: TableColumn<ClaimEvent>[] = [
  {
    accessorKey: "timestamp",
    header: "Time",
    cell: ({ row }) => new Date(row.original.timestamp).toLocaleString(),
  },
  { accessorKey: "event_type", header: "Transaction" },
  { accessorKey: "amount", header: "Coins" },
  {
    accessorKey: "treasury_after",
    header: "Balance after",
    cell: ({ row }) => formatAmount(row.original.treasury_after),
  },
  { accessorKey: "actor_entity_id", header: "Player" },
];
</script>

<template>
  <div class="flex flex-col gap-4">
    <UTabs
      v-model="activeGroup"
      :items="tabs"
      :content="false"
      variant="link"
      :ui="{ list: 'flex-wrap', trigger: 'shrink-0' }"
      aria-label="Claim event groups"
    />

    <div class="flex flex-wrap items-start justify-between gap-3">
      <div class="space-y-1">
        <h3 class="flex items-center gap-2 text-base font-semibold text-highlighted">
          <UIcon :name="groups[activeGroup].icon" class="size-5" />
          {{ groups[activeGroup].label }}
          <UBadge v-if="data && !pending && !error" color="neutral" variant="subtle">
            {{ formatAmount(data.total) }}
          </UBadge>
        </h3>
        <p class="max-w-2xl text-sm text-muted">{{ groups[activeGroup].description }}</p>
      </div>
      <UButton
        label="Refresh"
        icon="i-lucide-refresh-cw"
        variant="soft"
        :loading="pending"
        @click="refresh()"
      />
    </div>

    <UAlert v-if="error" color="error" title="Could not load claim events. Try refreshing." />
    <div v-else-if="pending" class="py-8" role="status" aria-label="Loading claim events">
      <UProgress color="neutral" />
    </div>
    <UEmpty
      v-else-if="!data?.events.length"
      :icon="groups[activeGroup].icon"
      :title="
        activeGroup === 'all'
          ? 'No claim events recorded yet'
          : `No ${groups[activeGroup].label.toLowerCase()} events recorded yet`
      "
      description="New activity will appear here after it has been recorded."
    />

    <UTable
      v-else-if="activeGroup === 'treasury'"
      :data="data?.events ?? []"
      :columns="treasuryColumns"
    >
      <template #event_type-cell="{ row }">
        <UBadge
          :color="eventStyles[row.original.event_type].color"
          variant="subtle"
          :icon="eventStyles[row.original.event_type].icon"
        >
          {{ eventStyles[row.original.event_type].label }}
        </UBadge>
      </template>
      <template #amount-cell="{ row }">
        <span
          class="font-semibold tabular-nums"
          :class="row.original.event_type === 'TreasuryWithdrawal' ? 'text-error' : 'text-success'"
        >
          {{ signedAmount(row.original) }}
        </span>
      </template>
      <template #actor_entity_id-cell="{ row }">
        <NuxtLink
          v-if="row.original.actor_entity_id !== null"
          :to="`/players/${row.original.actor_entity_id}`"
          class="text-primary hover:underline"
        >
          {{ playerName(row.original.actor_entity_id) }}
        </NuxtLink>
        <span v-else class="text-muted">Not recorded</span>
      </template>
    </UTable>

    <ol
      v-else
      :class="
        activeGroup === 'research'
          ? 'grid gap-3 lg:grid-cols-2'
          : 'space-y-3'
      "
      aria-label="Claim activity"
    >
      <li v-for="event in data?.events ?? []" :key="String(event.id)">
        <article
          class="flex h-full items-start gap-3 rounded-xl border border-default bg-default p-4"
        >
          <UBadge
            :color="eventStyles[event.event_type].color"
            variant="soft"
            class="shrink-0 rounded-lg p-2.5"
            aria-hidden="true"
          >
            <UIcon :name="eventStyles[event.event_type].icon" class="size-5" />
          </UBadge>
          <div class="min-w-0 flex-1 space-y-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
              <UBadge :color="eventStyles[event.event_type].color" variant="subtle">
                {{ eventStyles[event.event_type].label }}
              </UBadge>
              <time :datetime="event.timestamp" class="text-xs text-muted">{{
                new Date(event.timestamp).toLocaleString()
              }}</time>
            </div>

            <template v-if="eventStyles[event.event_type].group === 'treasury'">
              <p
                class="text-xl font-semibold tabular-nums"
                :class="event.event_type === 'TreasuryWithdrawal' ? 'text-error' : 'text-success'"
              >
                {{ signedAmount(event) }} <span class="text-sm font-normal text-muted">coins</span>
              </p>
              <p v-if="event.treasury_after !== null" class="text-sm text-muted">
                Balance after: {{ formatAmount(event.treasury_after) }} coins
              </p>
            </template>
            <template v-else>
              <NuxtLink
                v-if="event.member_entity_id !== null"
                :to="`/players/${event.member_entity_id}`"
                class="block font-semibold text-primary hover:underline"
              >
                {{ subject(event) }}
              </NuxtLink>
              <h4 v-else class="break-words font-semibold text-highlighted">
                {{ subject(event) }}
              </h4>

              <p
                v-if="eventStyles[event.event_type].group === 'research'"
                class="text-sm text-muted"
              >
                Claim research completed.
              </p>
              <p v-if="event.building_entity_id !== null" class="text-xs text-muted">
                Building ID: {{ event.building_entity_id }}
              </p>

              <div
                v-if="event.event_type === 'MemberPermissionsChanged'"
                class="flex flex-wrap gap-2"
              >
                <UBadge
                  v-for="change in permissionChanges(event)"
                  :key="change.label"
                  :color="change.granted ? 'success' : 'warning'"
                  variant="soft"
                  :icon="change.granted ? 'i-lucide-plus' : 'i-lucide-minus'"
                >
                  {{ change.label }} {{ change.granted ? "granted" : "revoked" }}
                </UBadge>
                <span v-if="!permissionChanges(event).length" class="text-xs text-muted"
                  >Permission details not recorded.</span
                >
              </div>
            </template>

            <p class="text-xs text-muted">
              <template v-if="event.actor_entity_id !== null">
                By
                <NuxtLink
                  :to="`/players/${event.actor_entity_id}`"
                  class="text-primary hover:underline"
                  >{{ playerName(event.actor_entity_id) }}</NuxtLink
                >
              </template>
              <template v-else>Acting player not recorded</template>
            </p>
          </div>
        </article>
      </li>
    </ol>

    <div
      v-if="!error && !pending && Number(data?.total ?? 0) > perPage"
      class="flex justify-center"
    >
      <UPagination
        v-model:page="page"
        :total="Number(data?.total ?? 0)"
        :items-per-page="perPage"
      />
    </div>
  </div>
</template>
