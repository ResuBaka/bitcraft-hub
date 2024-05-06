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

const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});
</script>

<template>
  <v-card>
      <template v-slot:title>
        <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'claims-id', params: { id: claim.entity_id } }"
        >{{ claim.name }} : {{ claim.entity_id }}</nuxt-link>
      </template>
    <v-card-text  :class="computedClass">
      <v-table :class="computedClass" density="compact">
        <tbody>
        <tr style='text-align: right'>
          <th>Owner:</th>
          <td>{{claim.owner_player_entity_id}}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Supplies:</th>
          <td>{{ parseInt(claim.supplies) }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Tiles:</th>
          <td>{{ claim.tiles }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Location:</th>
          <td>{{ claim.location[0][0] }} x {{ claim.location[0][1] }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Members:</th>
          <td v-if="claim.members.length > 0">
            <template v-for="member of claim.members">
              <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black" :to="{ name: 'players-id', params: { id: member.entity_id } }"
        >{{ member.user_name }}</nuxt-link>,
            </template>
          </td>
          <td v-else>None</td>
        </tr>
        </tbody>
      </v-table>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>