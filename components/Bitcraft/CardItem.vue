<script setup lang="ts">
const { item } = defineProps<{
  item: any;
}>();

const { data: neededInCrafting } = useFetch("/api/bitcraft/recipes", {
  query: {
    neededInCrafting: item.id,
  },
});

const { data: producedInCrafting } = useFetch("/api/bitcraft/recipes", {
  query: {
    producedInCrafting: item.id,
  },
});

const neededInCraftingData = computed(() => {
  return neededInCrafting.value ?? [];
});

const producedInCraftingData = computed(() => {
  return producedInCrafting.value ?? [];
});

const contetentToShow = ref("default");
</script>

<template>
  <v-card>
    <v-toolbar color="transparent">
      <v-toolbar-title>{{ item.name }} : {{ item.id }}</v-toolbar-title>

      <template v-slot:append>


        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'default'"
            >
              <v-icon >
                mdi-home
              </v-icon>
            </v-btn>
          </template>
          <span>Programmatic tooltip</span>
        </v-tooltip>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'neededInCrafting'"
            >
              <v-badge color="error" :content="neededInCraftingData.length">
                <v-icon >
                  mdi-call-split
                </v-icon>
              </v-badge>
            </v-btn>
          </template>
          <span>neededInCrafting</span>
        </v-tooltip>
        <v-tooltip
            location="bottom"
        >
          <template v-slot:activator="{ props }">
            <v-btn
                icon
                v-bind="props"
                @click="contetentToShow = 'producedInCrafting'"
            >
              <v-badge color="error" :content="producedInCraftingData.length">
                <v-icon >
                  mdi-call-split
                </v-icon>
              </v-badge>
            </v-btn>
          </template>
          <span>producedInCrafting</span>
        </v-tooltip>
      </template>
    </v-toolbar>

    <v-card-text>
      <template v-if="contetentToShow == 'default'">
        <v-list>
          <v-list-item>
            <v-list-item-title>Description</v-list-item-title>
            <v-list-item-subtitle>{{ item.description }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Volume</v-list-item-title>
            <v-list-item-subtitle>{{ item.volume }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Tag</v-list-item-title>
            <v-list-item-subtitle>{{ item.tag }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Tier</v-list-item-title>
            <v-list-item-subtitle>{{ item.tier }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Model Asset Name</v-list-item-title>
            <v-list-item-subtitle>{{ item.model_asset_name }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Icon Asset Name</v-list-item-title>
            <v-list-item-subtitle>{{ item.icon_asset_name }}</v-list-item-subtitle>
          </v-list-item>
        </v-list>
      </template>
      <template v-if="contetentToShow == 'neededInCrafting'">
        <v-list>
          <v-list-item v-for="crafting in neededInCraftingData" :key="crafting.id">
            <v-list-item-title>
              <bitcraft-card-item-crafting-name :item="item" :template="crafting.name"
                                                :craftId="crafting.crafted_item_stacks[0].item_id"></bitcraft-card-item-crafting-name>
            </v-list-item-title>
            <v-list-item-subtitle>actions_required: {{ crafting.actions_required }}</v-list-item-subtitle>
            <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>ItemId: {{ crafting.crafted_item_stacks[0].item_id }}</v-list-item-subtitle>
            <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
          </v-list-item>
        </v-list>
      </template>
      <template v-if="contetentToShow == 'producedInCrafting'">
        <v-list>
          <v-list-item v-for="crafting in producedInCraftingData" :key="crafting.id">
            <v-list-item-title>
              <bitcraft-card-item-crafting-name :item="item" :template="crafting.name"
                                                :craftId="crafting.crafted_item_stacks[0].item_id"></bitcraft-card-item-crafting-name>
            </v-list-item-title>
            <v-list-item-subtitle>actions_required: {{ crafting.actions_required }}</v-list-item-subtitle>
            <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
            <v-list-item-subtitle>ItemId: {{ crafting.crafted_item_stacks[0].item_id }}</v-list-item-subtitle>
            <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
          </v-list-item>
        </v-list>
      </template>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>