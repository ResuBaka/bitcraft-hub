<script setup lang="ts">
const { claim } = defineProps<{
  claim: any;
}>();

/*const {data: buidlings} = useFetch('/api/bitcraft/buildings', {
  query: {
    claim_entity_id: claim.entity_id
  }
})

const {data: owner_player} = useFetch('/api/bitcraft/player', {
  query: {
    entity_id: claim.owner_player_entity_id
  }
})

const buidlingsData = computed(() => {
  return buidlings.value ?? []
})

const ownerPlayerData = computed(() => {
  return owner_player.value ?? []
})*/
</script>

<template>
  <v-card>
      <template v-slot:title>
        <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'claims-id', params: { id: claim.entity_id } }"
        >{{ claim.name }} : {{ claim.entity_id }}</nuxt-link>
      </template>
    <v-card-text  class="bg-surface-light">
        <v-list class="bg-surface-light">
          <v-list-item>
            <v-list-item-title>Owner</v-list-item-title>
            <v-list-item-subtitle>{{ claim.owner_player_entity_id }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Supplies</v-list-item-title>
            <v-list-item-subtitle>{{ claim.supplies }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Tiles</v-list-item-title>
            <v-list-item-subtitle>{{ claim.tiles }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Location</v-list-item-title>
            <v-list-item-subtitle>{{ claim.location }}</v-list-item-subtitle>
          </v-list-item>
          <v-list-item>
            <v-list-item-title>Members</v-list-item-title>
            <v-list-item v-for="member in claim.members" :key="member.user_name">
              <v-list-item-subtitle>{{ member.user_name }}</v-list-item-subtitle>
          </v-list-item>
          </v-list-item>
        </v-list>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>