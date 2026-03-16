<script setup lang="ts">
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { ItemType } from "~/types/ItemType";
import type { PlayerUsernameStateResponse } from "~/types/PlayerUsernameStateResponse";

const props = withDefaults(
  defineProps<{
    items?: InventoryChangelog[];
  }>(),
  {
    items: () => [],
  },
);

const page = ref(1);
const pageSize = 50;

const tableRows = computed(() => props.items ?? []);

const pagedRows = computed(() => {
  const start = (page.value - 1) * pageSize;
  return tableRows.value.slice(start, start + pageSize);
});

const nDate = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  fractionalSecondDigits: 1,
  hour12: false,
});

const nUTCData = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
  timeZone: "UTC",
});

function timeAgo(date: number) {
  const seconds = Math.floor((Date.now() - date) / 1000);
  const interval = Math.floor(seconds / 31536000);
  if (interval > 1) {
    return interval + " years ago";
  }
  if (interval === 1) {
    return interval + " year ago";
  }
  const months = Math.floor(seconds / 2628000);
  if (months > 1) {
    return months + " months ago";
  }
  if (months === 1) {
    return months + " month ago";
  }
  const days = Math.floor(seconds / 86400);
  if (days > 1) {
    return days + " days ago";
  }
  if (days === 1) {
    return days + " day ago";
  }
  const hours = Math.floor(seconds / 3600);
  if (hours > 1) {
    return hours + " hours ago";
  }
  if (hours === 1) {
    return hours + " hour ago";
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes > 1) {
    return minutes + " minutes ago";
  }
  if (minutes === 1) {
    return minutes + " minute ago";
  }
  return "just now";
}

const { data: ItemAndCargoFetch } = useFetchMsPack<ItemsAndCargollResponse>(() => {
  return `/api/bitcraft/itemsAndCargo/all`;
});

const { data: PlayerUsernameStateFetch } = useFetchMsPack<PlayerUsernameStateResponse>(() => {
  return `/api/bitcraft/players/all`;
});

const columns = [
  { id: "user", header: "Player" },
  {
    id: "diff",
    header: "Diff",
    meta: { class: { th: "text-center", td: "text-center" } },
  },
  {
    id: "old_item_quantity",
    header: "Old Amount",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
  {
    id: "new_item_quantity",
    header: "New Amount",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
  {
    id: "timestamp",
    header: "Timestamp Since",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
  {
    id: "timestamp_diff",
    header: "Time Ago",
    meta: { class: { th: "text-right", td: "text-right" } },
  },
];

function getItemOrCargoName(item_id: number, item_type: ItemType) {
  if (ItemAndCargoFetch.value === undefined) {
    return;
  }
  if (item_type === "Item") {
    const itemDesc = ItemAndCargoFetch.value.item_desc[item_id];
    if (itemDesc === undefined) {
      return `${item_id}`;
    }
    return `${itemDesc.name} ${Array.from(itemDesc.rarity)[0]}`;
  } else {
    const cargoDesc = ItemAndCargoFetch.value.cargo_desc[item_id];
    if (cargoDesc === undefined) {
      return `${item_id}`;
    }
    return `${cargoDesc.name} ${Array.from(cargoDesc.rarity)[0]}`;
  }
}

function getUsername(user_id: bigint) {
  if (PlayerUsernameStateFetch.value === undefined || PlayerUsernameStateFetch.value === null) {
    return;
  }
  return PlayerUsernameStateFetch.value.username_state[user_id.toString()];
}

watch(
  () => props.items,
  () => {
    page.value = 1;
  },
);
</script>

<template>
  <div class="flex flex-col gap-3">
    <UTable class="inventory-changes-table" :columns="columns" :data="pagedRows">
      <template #user-cell="{ row }">
        <span v-if="row.original.user_id !== null">
          {{ getUsername(row.original.user_id) }}
        </span>
      </template>
      <template #timestamp-cell="{ row }">
        {{ nDate.format(Date.parse(row.original.timestamp)) }}
      </template>
      <template #timestamp_diff-cell="{ row }">
        <UTooltip>
          <template #content>
            UTC {{ nUTCData.format(Date.parse(row.original.timestamp)) }}
          </template>
          <span>{{ timeAgo(Date.parse(row.original.timestamp)) }}</span>
        </UTooltip>
      </template>
      <template #diff-cell="{ row }">
        <div class="diff-cell">
          <template
            v-if="
              row.original.type_of_change === 'Remove' &&
              row.original.old_item_id !== null &&
              row.original.old_item_type !== null
            "
          >
            <UIcon name="i-mdi-delete-empty" class="diff-icon text-red-500" />
            <strong>-{{ row.original.old_item_quantity }}</strong>
            {{ getItemOrCargoName(row.original.old_item_id, row.original.old_item_type) }}
          </template>
          <template
            v-else-if="
              row.original.type_of_change === 'Add' &&
              row.original.new_item_id !== null &&
              row.original.new_item_type !== null
            "
          >
            <UIcon name="i-mdi-plus" class="diff-icon text-emerald-500" />
            <strong>{{ row.original.new_item_quantity }}</strong>
            {{ getItemOrCargoName(row.original.new_item_id, row.original.new_item_type) }}
          </template>
          <template
            v-else-if="
              row.original.type_of_change === 'Update' &&
              row.original.new_item_id !== null &&
              row.original.new_item_type !== null &&
              row.original.old_item_quantity !== null &&
              row.original.new_item_quantity !== null &&
              row.original.old_item_quantity > row.original.new_item_quantity
            "
          >
            <UIcon name="i-mdi-arrow-up-bold-outline" class="diff-icon text-emerald-500" />
            <strong>{{ row.original.old_item_quantity - row.original.new_item_quantity }}</strong>
            {{ getItemOrCargoName(row.original.new_item_id, row.original.new_item_type) }}
          </template>
          <template
            v-else-if="
              row.original.type_of_change === 'Update' &&
              row.original.new_item_id !== null &&
              row.original.new_item_type !== null &&
              row.original.old_item_quantity !== null &&
              row.original.new_item_quantity !== null &&
              row.original.old_item_quantity < row.original.new_item_quantity
            "
          >
            <UIcon name="i-mdi-arrow-down-bold-outline" class="diff-icon text-red-500" />
            <strong>{{ row.original.old_item_quantity - row.original.new_item_quantity }}</strong>
            {{ getItemOrCargoName(row.original.new_item_id, row.original.new_item_type) }}
          </template>
          <template
            v-else-if="
              row.original.type_of_change === 'AddAndRemove' &&
              row.original.new_item_id !== null &&
              row.original.new_item_type !== null &&
              row.original.old_item_id !== null &&
              row.original.old_item_type !== null
            "
          >
            <strong class="text-red-500">
              {{ getItemOrCargoName(row.original.old_item_id, row.original.old_item_type) }}
            </strong>
            <UIcon name="i-mdi-swap-horizontal" class="diff-icon text-pink-400" />
            <strong class="text-emerald-500">
              {{ getItemOrCargoName(row.original.new_item_id, row.original.new_item_type) }}
            </strong>
          </template>
        </div>
      </template>
      <template #old_item_quantity-cell="{ row }">
        <span v-if="row.original.old_item_quantity !== null">
          {{ row.original.old_item_quantity }}
        </span>
      </template>
      <template #new_item_quantity-cell="{ row }">
        <span
          v-if="row.original.new_item_quantity !== null"
          :class="{
            'text-red-500':
              row.original.old_item_quantity !== null &&
              row.original.old_item_quantity - row.original.new_item_quantity < 0,
            'text-emerald-500':
              row.original.old_item_quantity !== null &&
              row.original.old_item_quantity - row.original.new_item_quantity > 0,
          }"
        >
          {{ row.original.new_item_quantity }}
        </span>
      </template>
    </UTable>
    <div v-if="tableRows.length > pageSize" class="flex justify-center">
      <UPagination v-model:page="page" :total="tableRows.length" :items-per-page="pageSize" />
    </div>
  </div>
</template>

<style scoped>
.diff-cell {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
}

.diff-icon {
  width: 16px;
  height: 16px;
}

:deep(.inventory-changes-table tbody tr:nth-child(odd)) {
  background: rgba(148, 163, 184, 0.06);
}

:deep(.inventory-changes-table tbody tr:hover) {
  background: rgba(148, 163, 184, 0.12);
}
</style>
