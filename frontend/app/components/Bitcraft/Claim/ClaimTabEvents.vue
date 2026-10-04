<script setup lang="ts">
import type { TableColumn } from "@nuxt/ui";
import type { ClaimEvent } from "~/types/ClaimEvent";
import type { ClaimEventsResponse } from "~/types/ClaimEventsResponse";

const props = defineProps<{
  claimId: string;
  region: number;
  members: Record<string, { user_name: string }>;
}>();

const page = ref(1);
const perPage = 20;
const { data, pending, error, refresh } = useFetchMsPack<ClaimEventsResponse>(
  () =>
    `/claims/${props.claimId}/events?region=${props.region}&page=${page.value}&per_page=${perPage}`,
);

watch(
  () => [props.claimId, props.region],
  () => {
    page.value = 1;
  },
);

const labels = {
  TreasuryDeposit: "Treasury deposit",
  TreasuryWithdrawal: "Treasury withdrawal",
  ResearchCompleted: "Research completed",
  BuildingPlaced: "Building placed",
  BuildingRemoved: "Building removed",
};

const columns: TableColumn<ClaimEvent>[] = [
  {
    accessorKey: "timestamp",
    header: "Time",
    cell: ({ row }) => new Date(row.original.timestamp).toLocaleString(),
  },
  {
    accessorKey: "event_type",
    header: "Event",
    cell: ({ row }) => labels[row.original.event_type],
  },
  {
    id: "details",
    header: "Details",
    cell: ({ row }) => {
      const event = row.original;
      if (event.amount !== null) {
        const sign = event.event_type === "TreasuryWithdrawal" ? "−" : "+";
        return `${sign}${Number(event.amount).toLocaleString()} coins`;
      }
      return (
        event.subject_name ??
        (event.research_id !== null
          ? `Research #${event.research_id}`
          : `Building #${event.building_description_id}`)
      );
    },
  },
  { accessorKey: "actor_entity_id", header: "Player" },
  {
    accessorKey: "treasury_after",
    header: "Treasury after",
    cell: ({ row }) =>
      row.original.treasury_after === null
        ? "—"
        : Number(row.original.treasury_after).toLocaleString(),
  },
];
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center justify-between gap-2">
      <p class="text-sm text-gray-500">
        Deposits, withdrawals, completed research and building changes. XP-generated coins are
        excluded. History starts when tracking is enabled.
      </p>
      <UButton
        label="Refresh"
        icon="i-heroicons-arrow-path"
        variant="soft"
        :loading="pending"
        @click="refresh()"
      />
    </div>
    <UAlert v-if="error" color="error" title="Could not load claim events. Try refreshing." />
    <UTable
      :data="data?.events ?? []"
      :columns="columns"
      :loading="pending"
      empty="No claim events recorded yet."
    >
      <template #actor_entity_id-cell="{ row }">
        <NuxtLink
          v-if="row.original.actor_entity_id !== null"
          :to="`/players/${row.original.actor_entity_id}`"
          class="text-primary"
        >
          {{
            members[String(row.original.actor_entity_id)]?.user_name ??
            `Player #${row.original.actor_entity_id}`
          }}
        </NuxtLink>
        <span v-else>—</span>
      </template>
    </UTable>
    <div class="flex justify-center">
      <UPagination
        v-model:page="page"
        :total="Number(data?.total ?? 0)"
        :items-per-page="perPage"
      />
    </div>
  </div>
</template>
