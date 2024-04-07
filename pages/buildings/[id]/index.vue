<script setup lang="ts">
import type { Building } from "~/types";

const route = useRoute();

const { data, refresh, pending } = useFetch<Building>(
  `/api/buildings/${route.params.id}`,
);

const headers = ref([
  { title: "Name", key: "name" },
  { title: "Tier", key: "tier" },
  { title: "Requirement", key: "requirement" },
  { title: "Recipes", key: "recipes", sortable: false },
  { title: "Actions", key: "actions", sortable: false, align: "end" },
]);
const search = ref("");

const building = computed<Building>(() => {
  return (
    data.value || {
      name: "",
      tier: "1",
      requirement: [],
      recipes: [],
      toCraft: [],
      id: "",
    }
  );
});

const dev = import.meta.dev;
</script>

<template>
  <template v-if="pending">
    <v-container>
      <v-progress-linear indeterminate color="primary"></v-progress-linear>
    </v-container>
  </template>
  <template v-else-if="!data">
    <v-container>
      <v-alert variant="outlined" class="ma-5 justify-center">Building not found.</v-alert>
      <v-row justify="center">
        <v-col cols="auto">
          <v-btn :to="{ name: 'buildings' }">Go back to Buildings</v-btn>
        </v-col>
      </v-row>
    </v-container>
  </template>
  <template v-else>
    <v-container>
      <v-card>
        <v-card-title>
          <v-row>
            <v-col >
              <span class="text-h5">Building: {{ building.name }}</span>
            </v-col>
            <v-spacer></v-spacer>
            <v-col cols="auto" align-self="end">
              <v-btn :to="{ name: 'buildings-id-edit', params: { id: building.id } }">Edit</v-btn>
            </v-col>
          </v-row>
        </v-card-title>

        <v-card-text>
          <v-row>
            <v-col>Id: {{ building.id }}</v-col>
            <v-col>Tier: {{ building.tier }}</v-col>
          </v-row>
          <v-row v-if="dev">
            <v-expansion-panels>
              <v-expansion-panel title="Json">
                <v-expansion-panel-text>
                  <pre>{{ JSON.stringify(building, null, 4) }}</pre>
                </v-expansion-panel-text>
              </v-expansion-panel>
            </v-expansion-panels>
          </v-row>
        </v-card-text>
      </v-card>
    </v-container>
    <v-container>
      <v-card>
        <v-card-title>
          <span class="text-h5 ">Requirement</span>
        </v-card-title>
        <v-card-text>
        </v-card-text>
      </v-card>
    </v-container>
    <v-container>
      <v-card>
        <v-card-title>
          <span class="text-h5">To Craft</span>
        </v-card-title>
        <v-card-text>
        </v-card-text>
      </v-card>
    </v-container>
    <v-container>
      <v-card>
        <v-card-title>
          <span class="text-h5">Items it can craft</span>
        </v-card-title>

        <v-card-text>
        </v-card-text>
      </v-card>
    </v-container>
  </template>
</template>
