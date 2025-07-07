<script setup lang="ts">
import type { InventoryChangelog } from "~/types/InventoryChangelog";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { ItemType } from "~/types/ItemType";
import type { PlayerUsernameStateResponse } from "~/types/PlayerUsernameStateResponse";

const { items } = defineProps<{
  items: InventoryChangelog[];
}>();

const nDate = Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
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

const { data: ItemAndCargoFetch } = useFetchMsPack<ItemsAndCargollResponse>(
  () => {
    return `/api/bitcraft/itemsAndCargo/all`;
  },
);

const { data: PlayerUsernameStateFetch } =
  useFetchMsPack<PlayerUsernameStateResponse>(() => {
    return `/api/bitcraft/players/all`;
  });

const headersChanges = [
  { title: "Player", key: "user", align: "start" },
  { title: "Diff", key: "diff", align: "center" },
  {
    title: "Old Amount",
    key: "old_item_quantity",
    align: "end",
    maxWidth: "100px",
  },
  {
    title: "New Amount",
    key: "new_item_quantity",
    align: "end",
    maxWidth: "100px",
  },
  {
    title: "Timestamp Since",
    key: "timestamp",
    align: "end",
    maxWidth: "100px",
  },
  {
    title: "Time Ago",
    key: "timestamp_diff",
    align: "end",
    maxWidth: "100px",
  },
];

function getItemOrCargoName(item_id: number, item_type: ItemType) {
  if (ItemAndCargoFetch.value === undefined) {
    return;
  }
  if (item_type === "Item") {
    const itemDesc = ItemAndCargoFetch.value.item_desc[item_id];
    if (itemDesc == undefined) {
      return `${item_id}`;
    }
    return `${itemDesc.name} ${Array.from(itemDesc.rarity)[0]}`;
  } else {
    const cargoDesc = ItemAndCargoFetch.value.cargo_desc[item_id];
    if (cargoDesc == undefined) {
      return `${item_id}`;
    }
    return `${cargoDesc.name} ${Array.from(cargoDesc.rarity)[0]}`;
  }
}

function getUsername(user_id: bigint) {
  if (
    PlayerUsernameStateFetch.value === undefined ||
    PlayerUsernameStateFetch.value === null
  ) {
    return;
  }
  return PlayerUsernameStateFetch.value.username_state[user_id.toString()];
}

const backgroundColorRow = ({ index }: { index: number }) => {
  return {
    class: index % 2 === 0 ? "" : "bg-surface-light",
  };
};
</script>


<template>
<v-data-table density="compact" :headers="headersChanges" :items="items" :row-props="backgroundColorRow">
            <template v-slot:item.user="{ item } ">
               <template v-if="item.user_id !== null">
                    {{ getUsername(item.user_id) }}
               </template>
            </template>
            <template v-slot:item.timestamp="{ item }">
              {{ nDate.format(Date.parse(item.timestamp)) }}
            </template>
            <template v-slot:item.timestamp_diff="{ item }">
              <v-tooltip :text="`UTC ${ nUTCData.format(Date.parse(item.timestamp)) }`" location="top">
                <template v-slot:activator="{ props }">
                  <div v-bind="props" >{{ timeAgo( Date.parse(item.timestamp)) }} </div>
                </template>
              </v-tooltip>
            </template>
            <template v-slot:item.diff="{ item }">
              <template v-if="item.type_of_change === 'Remove' && item.old_item_id !== null && item.old_item_type !== null">
                <v-icon color="red">mdi-delete-empty</v-icon>
                <b>-{{ item.old_item_quantity }}</b> {{ getItemOrCargoName(item.old_item_id,item.old_item_type) }}
              </template>
              <template v-if="item.type_of_change === 'Add' && item.new_item_id !== null && item.new_item_type !== null">
                <v-icon color="green">mdi-plus</v-icon>
                <b>{{ item.new_item_quantity }}</b> {{ getItemOrCargoName(item.new_item_id,item.new_item_type)  }}
              </template>
              <template v-if="item.type_of_change === 'Update' && item.new_item_id !== null && item.new_item_type !== null && item.old_item_quantity !== null && item.new_item_quantity !== null && item.old_item_quantity > item.new_item_quantity">
                <v-icon color="green">mdi-arrow-up-bold-outline</v-icon>
                <b>{{ item.old_item_quantity - item.new_item_quantity }}</b> {{ getItemOrCargoName(item.new_item_id,item.new_item_type) }}
              </template>
              <template v-if="item.type_of_change === 'Update' && item.new_item_id !== null && item.new_item_type !== null && item.old_item_quantity !== null && item.new_item_quantity !== null && item.old_item_quantity < item.new_item_quantity">
                <v-icon color="red">mdi-arrow-down-bold-outline</v-icon>
                <b>{{ item.old_item_quantity - item.new_item_quantity }}</b> {{  getItemOrCargoName(item.new_item_id,item.new_item_type)  }}
              </template>
              <template v-if="item.type_of_change === 'AddAndRemove' && item.new_item_id !== null && item.new_item_type !== null"><b class="text-red">{{ getItemOrCargoName(item.old_item_id,item.old_item_type)  }}</b>
                <v-icon color="pink">mdi-swap-horizontal</v-icon>
                <b class="text-green">{{ getItemOrCargoName(item.new_item_id,item.new_item_type) }}</b></template>
            </template>
            <template v-slot:item.diff.old="{item } ">
              <template v-if="item.old_item_id !== null && item.old_item_quantity">{{ item.old_item_quantity }}</template>
            </template>
            <template v-slot:item.diff.new="{item }">
              <template v-if="item.old_item_quantity !== null && item.new_item_quantity !== null">
                <div :class="{ 'text-red': item.old_item_quantity - item.new_item_quantity < 0, 'text-green': item.old_item_quantity - item.new_item_quantity > 0 }">
                  {{ item.new_item_quantity }}
                </div>
              </template>
            </template>
          </v-data-table>
</template>

<style scoped>
</style>