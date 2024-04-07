<script setup lang="ts">
import type { Building, Item, Profession, Requirement } from "~/types";

const route = useRoute();

interface EditBuilding extends Building {
  edit: boolean;
}

const {
  data,
  refresh: refreshBuilding,
  pending,
} = useFetch<Building>(`/api/buildings/${route.params.id}`);

const refresh = () => {
  refreshItems();
  refreshProfessions();
  refreshBuilding();

  isInRequirementInEditMode.value.clear();
  requirementInEditMode.value.clear();
};

const { data: professions, refresh: refreshProfessions } =
  useFetch<Profession[]>(`/api/professions`);

const { data: items, refresh: refreshItems } = useFetch<Item[]>(`/api/items`);

const requirementInEditMode = ref(new Map<string, Requirement>());
const isInRequirementInEditMode = ref(new Set<string>());

const requirementHeaders = ref([
  { title: "Name", key: "id" },
  { title: "Type", key: "type" },
  { title: "Level", key: "level" },
  { title: "Actions", key: "actions", sortable: false, align: "end" },
]);

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

const types = ["item", "profession"];

const requirementIdToName = (requirement: Requirement) => {
  if (requirement.type === "item") {
    if (!items.value) {
      return null;
    }
    const itemData = items.value.find((i) => i.id === requirement.id);

    return itemData?.name || "Unknown ";
  }

  if (requirement.type === "profession") {
    if (!professions.value) {
      return null;
    }
    const itemData = professions.value.find((i) => i.id === requirement.id);

    return itemData?.id || "Unknown";
  }
};

const editRequirementHasChanges = computed(() => {
  const changed = new Set<string>();

  for (const requirementItem of building.value.requirement) {
    if (!requirementItem) {
      continue;
    }

    if (!requirementInEditMode.value.has(requirementItem.uuid)) {
      continue;
    }

    const original = requirementInEditMode.value.get(requirementItem.uuid);
    if (!original) {
      continue;
    }

    if (original.type !== requirementItem.type) {
      changed.add(requirementItem.uuid);
    }

    if (original.level != requirementItem.level) {
      changed.add(requirementItem.uuid);
    }

    if (original.id !== requirementItem.id) {
      changed.add(requirementItem.uuid);
    }
  }

  return changed;
});

const search = ref("");

const toggleEditRequirement = (requirement: Requirement) => {
  if (requirementInEditMode.value.has(requirement.uuid)) {
    if (!editRequirementHasChanges.value.has(requirement.uuid)) {
      requirementInEditMode.value.delete(requirement.uuid);
    }
    isInRequirementInEditMode.value.delete(requirement.uuid);
  } else {
    requirementInEditMode.value.set(
      requirement.uuid,
      Object.assign({}, requirement),
    );
    isInRequirementInEditMode.value.add(requirement.uuid);
  }
};

const deleteRequirement = (requirement: Requirement) => {
  if (!confirm("Are you sure you want to delete this requirement?")) {
    return;
  }

  const { status, data } = useFetch(
    `/api/buildings/${building.value.id}/requirement/${requirement.uuid}`,
    {
      method: "DELETE",
    },
  );

  if (requirementInEditMode.value.has(requirement.uuid)) {
    requirementInEditMode.value.delete(requirement.uuid);
  }

  refresh();
};

const saveRequirement = (requirement: Requirement) => {
  if (!confirm("Are you sure you want to delete this requirement?")) {
    return;
  }

  const { status, data } = useFetch(
    `/api/buildings/${building.value.id}/requirement/${requirement.uuid}`,
    {
      method: "PUT",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        ...requirement,
        level: Number(requirement.level),
      }),
    },
  );

  if (requirementInEditMode.value.has(requirement.uuid)) {
    requirementInEditMode.value.delete(requirement.uuid);
  }

  if (isInRequirementInEditMode.value.has(requirement.uuid)) {
    isInRequirementInEditMode.value.delete(requirement.uuid);
  }

  refresh();
};

const addRequirement = () => {
  const requirement = {
    type: "item",
    level: 1,
    id: "",
    uuid: crypto.randomUUID(),
  };

  requirementInEditMode.value.set(
    requirement.uuid,
    Object.assign(
      {},
      {
        ...requirement,
        level: -100000,
      },
    ),
  );
  isInRequirementInEditMode.value.add(requirement.uuid);
  data.value?.requirement.push(requirement);
};

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
            <v-data-table
                :headers="requirementHeaders"
                :items="building.requirement"
                item-value="uuid"
                :sort-by="[{ key: 'name', order: 'asc' }]"
            >
              <template #top>
                <v-toolbar
                    flat
                >
                  <v-toolbar-title>Requirement</v-toolbar-title>
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
                  <v-btn
                      class="mb-2"
                      color="primary"
                      dark
                      icon="mdi-plus"
                      @click="addRequirement"
                  >
                  </v-btn>
                </v-toolbar>
              </template>
              <template #[`item.type`]="{ item }">
                <template v-if="item && !isInRequirementInEditMode.has(item.uuid)">
                  {{ item.type }}
                </template>
                <template v-else-if="item">
                  <v-select
                      v-model="item.type"
                      :items="types"
                      item-text="name"
                      item-value="id"
                  >
                  </v-select>
                </template>
              </template>
              <template #[`item.level`]="{ item }">
                <template v-if="item && !isInRequirementInEditMode.has(item.uuid)">
                  {{ item.level }}
                </template>
                <template v-else-if="item">
                  <v-text-field type="number"
                      v-model="item.level"
                      label="Level"
                  >
                  </v-text-field>
                </template>
              </template>
              <template #[`item.id`]="{ item }">
                <template v-if="item && !isInRequirementInEditMode.has(item.uuid)">
                  {{ requirementIdToName(item) }}
                </template>
                <template v-else-if="item && item.type == 'item'">
                  <v-select
                      v-model="item.id"
                      :items="items.map((i) => ({ id: i.id, title: i.name }))"
                      item-text="name"
                      item-value="id"
                  >
                  </v-select>
                </template>
                <template v-else-if="item && item.type == 'profession'">
                  <v-select
                      v-model="item.id"
                      :items="professions.map((i) => ({ id: i.id, title: i.id }))"
                      item-text="name"
                      item-value="id"
                  >
                  </v-select>
                </template>
              </template>
              <template #item.actions="{ item }">
                <v-icon
                    size="small"
                    v-if="item && editRequirementHasChanges.has(item.uuid)"
                    @click="saveRequirement(item)"
                >
                  mdi-content-save
                </v-icon>
                <v-icon
                    size="small"
                    @click="toggleEditRequirement(item)"
                >
                  mdi-pencil
                </v-icon>
                <v-icon
                    class="me-1"
                    size="small"
                    @click="deleteRequirement(item)"
                >
                  mdi-delete
                </v-icon>
              </template>
              <template #no-data>
                <v-alert variant="outlined" class="ma-5">No Buildings, refresh or add a building.</v-alert>
              </template>
            </v-data-table>
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
