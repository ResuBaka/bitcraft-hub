<script setup lang="ts">
const page = ref(1);
const perPage = 30;

const search = ref<string | undefined>("");
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
  { title: "Player", key: "playerName", align: "start" },
  { title: "Timestamp Local", key: "timestamp", align: "start" },
  { title: "Timestamp UTC", key: "timestamp_utc", align: "start" },
  { title: "Diff", key: "diff", align: "center" },
  { title: "Old Amount", key: "diff.old", align: "end" },
  { title: "New Amount", key: "diff.new", align: "end" },
];

const changes = computed(() => {
  const changes = [];
  for (const change of inventoryChanges.value) {
    const data = new Date(change.timestamp / 1000);
    if (change.diff) {
      for (const diff in change.diff) {
        let newDiff = undefined;
        let oldDiff = undefined;
        if (change.diff[diff].new !== undefined) {
          newDiff = change.diff[diff].new;
        }
        if (change.diff[diff].old !== undefined) {
          oldDiff = change.diff[diff].old;
        }

        let type = "";
        let amountDiff = 0;

        if (newDiff !== undefined && oldDiff !== undefined) {
          amountDiff = newDiff.quantity - oldDiff.quantity;
          type = amountDiff > 0 ? "increase" : "decrease";
        } else if (newDiff !== undefined) {
          type = "create";
          amountDiff = newDiff.quantity;
        } else if (oldDiff !== undefined) {
          type = "delete";
          amountDiff = oldDiff.quantity;
        }

        changes.push({
          playerName: change.playerName,
          timestamp: data,
          timestamp_utc: data,
          diff: {
            type: type,
            old: oldDiff,
            new: newDiff,
            amountDiff: amountDiff,
            item: {
              name: newDiff?.item?.name ?? oldDiff?.item?.name,
              id: newDiff?.item?.id ?? oldDiff?.item?.id,
            },
          },
        });
      }
    }
  }
  return (
    changes.filter((change) => {
      return (
        !search.value ||
        change?.diff?.new?.item?.name
          ?.toLowerCase()
          .includes(search.value.toLowerCase()) ||
        change?.diff?.old?.item?.name
          ?.toLowerCase()
          .includes(search.value.toLowerCase())
      );
    }) ?? []
  );
});

const backgroundColorRow = ({ index }) => {
  return {
    class: index % 2 === 0 ? "" : "bg-surface-light",
  };
};
</script>

<template>
  <template  v-if="inventory !== undefined">
    <v-card class="mb-5">
    <v-toolbar color="transparent">
      <v-toolbar-title >Inventory: {{ inventory.entity_id }}</v-toolbar-title>

    </v-toolbar>

    <v-card-text>
      <v-card-title>Current Items</v-card-title>
      <v-data-table density="compact" :headers="headersPockets" :items="inventory.pockets.filter((item) => !!item.contents)" :row-props="backgroundColorRow">
      </v-data-table>
    </v-card-text>
    </v-card>
    <v-spacer></v-spacer>
    <v-card>
      <v-card-title>Changes</v-card-title>
      <v-card-text>
        <v-row>
    <v-col>
      <v-text-field
          v-model="search"
          label="Search"
          outlined
          dense
          clearable
      ></v-text-field>
    </v-col>
  </v-row>
        <v-data-table density="compact" :headers="headersChanges" :items="changes" :row-props="backgroundColorRow">
          <template v-slot:item.timestamp="{ item }">
            {{ nDate.format(item.timestamp) }}
          </template>
          <template v-slot:item.timestamp_utc="{ item }">
            {{ nUTCData.format(item.timestamp) }}
          </template>
          <template v-slot:item.diff="{ value }">
            <template v-if="value.type === 'delete'"><v-icon color="red">mdi-delete-empty</v-icon> <b>{{ value.amountDiff }}</b> {{ value.item.name }}</template>
            <template v-if="value.type === 'create'"><v-icon color="green">mdi-plus</v-icon> <b>{{ value.amountDiff }}</b> {{ value.item.name }}</template>
            <template v-if="value.type === 'increase'"><v-icon color="green">mdi-arrow-up-bold-outline</v-icon> <b>{{ value.amountDiff }}</b> {{ value.item.name }}</template>
            <template v-if="value.type === 'decrease'"><v-icon color="red">mdi-arrow-down-bold-outline</v-icon> <b>{{ value.amountDiff }}</b> {{ value.item.name }}</template>
          </template>
          <template v-slot:item.diff.old="{ value, item }">
            <template v-if="value !== undefined">{{ value.quantity }}</template>
          </template>
          <template v-slot:item.diff.new="{ value, item }">
            <template v-if="value !== undefined"><div :class="{ 'text-red': item.diff.amountDiff < 0, 'text-green': item.diff.amountDiff > 0 }">{{ value.quantity }}</div></template>
          </template>
        </v-data-table>
      </v-card-text>
    </v-card>
  </template>
</template>

<style scoped>
</style>