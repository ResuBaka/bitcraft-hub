<script setup lang="ts">
import type { Building } from "~/types";

const { data, refresh, pending } = useFetch<Building[]>("/api/buildings");

const defaultItem: Building = {
  name: "",
  tier: "1",
  requirement: [],
  recipes: [],
  toCraft: [],
  id: "",
};

const headers = ref([
  { title: "Name", key: "name" },
  { title: "Tier", key: "tier" },
  { title: "Requirement", key: "requirement" },
  { title: "Recipes", key: "recipes", sortable: false },
  { title: "Actions", key: "actions", sortable: false, align: "end" },
]);
const search = ref("");

const buildings = computed(
  () =>
    data.value?.map((item) => ({
      ...item,
      recipes: item.recipes.length,
      requirement: item.requirement.length,
      toCraft: item.toCraft.length,
    })) || [],
);

const finialBuildings = computed(() => {
  let items = buildings.value;

  if (search.value) {
    items = buildings.value.filter((item) =>
      item.name.toLowerCase().includes(search.value.toLowerCase()),
    );
  }

  return items;
});

const dialog = ref(false);
const dialogDelete = ref(false);
const editedIndex = ref(-1);
const editedItem = ref<Building>({ ...defaultItem });
const editedError = ref("");

const formTitle = computed(() => {
  return editedIndex.value === -1 ? "New Item" : "Edit Item";
});

const editItem = (item: Building) => {
  editedIndex.value = data.value.findIndex((i) => i.id === item.id);
  editedItem.value = Object.assign({}, data.value[editedIndex.value]);
  dialog.value = true;
};

const deleteItem = (item: Building) => {
  editedIndex.value = data.value.findIndex((i) => i.id === item.id);
  editedItem.value = Object.assign({}, data.value[editedIndex.value]);
  dialogDelete.value = true;
};

const deleteItemConfirm = async () => {
  const { status } = await useFetch(`/api/buildings/${editedItem.value.id}`, {
    method: "DELETE",
  });

  if (status.value !== "success") {
    editedError.value = "An error happened while deleting building";
    return;
  }

  refresh();
  closeDelete();
};

const close = () => {
  dialog.value = false;
  editedError.value = "";
  nextTick(() => {
    editedItem.value = Object.assign({}, defaultItem);
    editedIndex.value = -1;
  });
};

const closeDelete = () => {
  dialogDelete.value = false;
  editedError.value = "";
  nextTick(() => {
    editedItem.value = Object.assign({}, defaultItem);
    editedIndex.value = -1;
  });
};

const save = async () => {
  const { status } = await useFetch(`/api/buildings/${editedItem.value.id}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(editedItem.value),
  });

  if (status.value !== "success") {
    editedError.value = "An error happened while deleting building";
    return;
  }

  refresh();
  close();
};
</script>

<template>
  <v-container>
  <v-data-table
      :headers="headers"
      :items="finialBuildings"
      :sort-by="[{ key: 'name', order: 'asc' }]"
  >
    <template v-slot:top>
      <v-toolbar
          flat
      >
        <v-toolbar-title>Buildings</v-toolbar-title>
        <v-divider
            class="mx-4"
            inset
            vertical
        ></v-divider>
        <v-text-field
            v-model="search"
            append-icon="mdi-magnify"
            label="Search"
            single-line
            hide-details
        >
        </v-text-field>
        <v-spacer></v-spacer>
        <v-btn
            class="mb-2"
            color="primary"
            dark
            icon="mdi-refresh"
            :loading="pending"
            @click="refresh"
        >
        </v-btn>
        <v-dialog
            v-model="dialog"
            max-width="500px"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                class="mb-2"
                color="primary"
                dark
                v-bind="props"
            >
              New Item
            </v-btn>
          </template>
          <v-card>
            <v-card-title>
              <span class="text-h5">{{ formTitle }}</span>
            </v-card-title>

            <v-card-text>
              <v-container>
                <v-row>
                  <v-col
                      cols="12"
                  >
                    <v-text-field
                        v-model="editedItem.name"
                        label="Building name"
                    ></v-text-field>
                  </v-col>
                  <v-col
                      cols="12"
                      md="4"
                      sm="6"
                  >
                    <v-text-field
                        v-model="editedItem.tier"
                        label="Tier"
                    ></v-text-field>
                  </v-col>
                </v-row>
              </v-container>
            </v-card-text>

            <v-card-actions>
              <v-spacer></v-spacer>
              <v-btn
                  color="blue-darken-1"
                  variant="text"
                  @click="close"
              >
                Cancel
              </v-btn>
              <v-btn
                  color="blue-darken-1"
                  variant="text"
                  @click="save"
              >
                Save
              </v-btn>
            </v-card-actions>
          </v-card>
        </v-dialog>
        <v-dialog v-model="dialogDelete" max-width="500px">
          <v-card>
            <v-card-title class="text-h5">Are you sure you want to delete this item?</v-card-title>
            <v-card-subtitle>{{ editedItem.name }}</v-card-subtitle>
            <v-card-actions>
              <v-spacer></v-spacer>
              <v-btn color="blue-darken-1" variant="text" @click="closeDelete">Cancel</v-btn>
              <v-btn color="blue-darken-1" variant="text" @click="deleteItemConfirm">OK</v-btn>
              <v-spacer></v-spacer>
            </v-card-actions>
          </v-card>
        </v-dialog>
      </v-toolbar>
    </template>
    <template v-slot:item.actions="{ item }">
      <nuxt-link style="text-decoration: none; color: inherit;" :to="{ name: 'buildings-id', params: { id: item.id } }">
        <v-icon
            class="me-1"
            size="small"
        >
          mdi-eye
        </v-icon>
      </nuxt-link>
      <v-icon
          size="small"
          @click="editItem(item)"
      >
        mdi-pencil
      </v-icon>
      <v-icon
          class="me-1"
          size="small"
          @click="deleteItem(item)"
      >
        mdi-delete
      </v-icon>
    </template>
    <template v-slot:no-data>
      <v-alert variant="outlined" class="ma-5">No Buildings, refresh or add a building.</v-alert>
    </template>
  </v-data-table>
  </v-container>
</template>
