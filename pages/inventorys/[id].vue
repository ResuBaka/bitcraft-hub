<script setup lang="ts">
const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");
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

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: inventoryFetch, pending: InventoryPending } = useFetch(() => {
  console.log(`/api/bitcraft/inventorys/${route.params.id}`);
  return `/api/bitcraft/inventorys/${route.params.id}`;
});

const { data: InventoryChangesFetch, pending: InventoryChangesPending } =
  useFetch(() => {
    console.log(`/api/bitcraft/inventorys/changes${route.params.id}`);
    return `/api/bitcraft/inventorys/changes/${route.params.id}`;
  });

const inventory = computed(() => {
  return inventoryFetch.value ?? undefined;
});
const inventoryChanges = computed(() => {
  return InventoryChangesFetch.value ?? [];
});

const headersPockets = [
  { title: "Name", key: "contents.item.name" },
  { title: "Quantity", key: "contents.quantity", align: "end" },
];

const headersChanges = [
  { title: "Player", key: "playerName" },
  { title: "Timestamp Local", key: "timestamp" },
  { title: "Timestamp UTC", key: "timestamp_utc" },
  {
    title: "New",
    align: "center",
    children: [
      { title: "New Quantity", key: "diff.new.quantity" },
      { title: "New Name", key: "diff.new.item.name" },
    ],
  },
  {
    title: "Old",
    align: "center",
    children: [
      { title: "Old Quantity", key: "diff.old.quantity" },
      { title: "Old Name", key: "diff.old.item.name" },
    ],
  },
];

const changes = computed(() => {
  return inventoryChanges.value.map((change) => {
    const data = new Date(change.timestamp / 1000);
    let newDiff = undefined;
    let oldDiff = undefined;

    if (change.diff) {
      for (const diff in change.diff) {
        if (change.diff[diff].new !== undefined) {
          newDiff = change.diff[diff].new;
        }
        if (change.diff[diff].old !== undefined) {
          oldDiff = change.diff[diff].old;
        }
      }
    }

    return {
      playerName: change.playerName,
      timestamp: data,
      timestamp_utc: data,
      diff: {
        new: newDiff,
        old: oldDiff,
      },
    };
  });
});
</script>

<template>
  <template  v-if="inventory !== undefined">
    <v-card class="mb-5">
    <v-toolbar color="transparent">
      <v-toolbar-title >Inventory: {{ inventory.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
      <v-card-title>Current Items</v-card-title>
      <v-data-table :headers="headersPockets" :items="inventory.pockets.filter((item) => !!item.contents)">
      </v-data-table>
    </v-card-text>
    </v-card>
    <v-spacer></v-spacer>
    <v-card>
      <v-card-title>Changes</v-card-title>
      <v-card-text>
        <v-data-table :headers="headersChanges" :items="changes">
          <template v-slot:item.timestamp="{ item }">
            {{ nDate.format(item.timestamp) }}
          </template>
          <template v-slot:item.timestamp_utc="{ item }">
            {{ nUTCData.format(item.timestamp) }}
          </template>
        </v-data-table>
      </v-card-text>
    </v-card>
  </template>
</template>

<style scoped>
</style>