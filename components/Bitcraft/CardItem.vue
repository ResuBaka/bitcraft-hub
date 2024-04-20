<script setup lang="ts">

const { item } = defineProps<{
  item: any
}>()

const { data: neededInCrafting } = useFetch('/api/bitcraft', {
  query: {
    neededInCrafting: item.id
  }
})

const { data: producedInCrafting } = useFetch('/api/bitcraft', {
  query: {
    producedInCrafting: item.id
  }
})

const neededInCraftingData = computed(() => {
  return neededInCrafting.value ?? []
})

const producedInCraftingData = computed(() => {
  return producedInCrafting.value ?? []
})

</script>

<template>
  <v-card>
    <v-card-title>{{ item.name }} : {{ item.id }}</v-card-title>
    <v-card-text>
      <v-list>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Description</v-list-item-title>
            <v-list-item-subtitle>{{ item.description }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Volume</v-list-item-title>
            <v-list-item-subtitle>{{ item.volume }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Tag</v-list-item-title>
            <v-list-item-subtitle>{{ item.tag }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Tier</v-list-item-title>
            <v-list-item-subtitle>{{ item.tier }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Model Asset Name</v-list-item-title>
            <v-list-item-subtitle>{{ item.model_asset_name }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-list-item>
          <v-list-item-content>
            <v-list-item-title>Icon Asset Name</v-list-item-title>
            <v-list-item-subtitle>{{ item.icon_asset_name }}</v-list-item-subtitle>
          </v-list-item-content>
        </v-list-item>
        <v-expansion-panels multiple>
          <v-expansion-panel>
          <v-expansion-panel-title>Needed In Crafting</v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-list>
              <v-list-item v-for="crafting in neededInCraftingData" :key="crafting.id">
                <v-list-item-title><bitcraft-card-item-crafting-name :item="item" :template="crafting.name" :craftId="crafting.crafted_item_stacks[0].item_id" ></bitcraft-card-item-crafting-name></v-list-item-title>
                <v-list-item-subtitle>actions_required: {{ crafting.actions_required }}</v-list-item-subtitle>
                <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
                <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
                <v-list-item-subtitle>ItemId: {{ crafting.crafted_item_stacks[0].item_id }}</v-list-item-subtitle>
                <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
              </v-list-item>
            </v-list>
          </v-expansion-panel-text>
        </v-expansion-panel>
        <v-expansion-panel>
          <v-expansion-panel-title>Produced In Crafting</v-expansion-panel-title>
          <v-expansion-panel-text>
            <v-list>
              <v-list-item v-for="crafting in producedInCraftingData" :key="crafting.id">
                <v-list-item-title><bitcraft-card-item-crafting-name :item="item" :template="crafting.name" :craftId="crafting.crafted_item_stacks[0].item_id" ></bitcraft-card-item-crafting-name></v-list-item-title>
                <v-list-item-subtitle>actions_required: {{ crafting.actions_required }}</v-list-item-subtitle>
                <v-list-item-subtitle>time_requirement: {{ crafting.time_requirement }}</v-list-item-subtitle>
                <v-list-item-subtitle>stamina_requirement: {{ crafting.stamina_requirement }}</v-list-item-subtitle>
                <v-list-item-subtitle>ItemId: {{ crafting.crafted_item_stacks[0].item_id }}</v-list-item-subtitle>
                <v-list-item-subtitle>Experience: {{ crafting.completion_experience[0].quantity }}</v-list-item-subtitle>
              </v-list-item>
            </v-list>
          </v-expansion-panel-text>
        </v-expansion-panel>
        </v-expansion-panels>
      </v-list>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>