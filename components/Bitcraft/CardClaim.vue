<script setup lang="ts">
export interface CardClaimProps {
  claim: any;
  defaultMembers: number;
}

const showMoreMembers = ref(false);

const { claim, defaultMembers } = withDefaults(defineProps<CardClaimProps>(), {
  defaultMembers: 10,
});

const members = computed(() => {
  return claim.members.slice(
    0,
    showMoreMembers.value ? claim.members.length : defaultMembers,
  );
});

const toggleShowMoreMembers = () => {
  showMoreMembers.value = !showMoreMembers.value;
};

const shouldShowMoreMembers = computed(() => {
  return claim.members.length > defaultMembers;
});

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
    <v-card-title>
      <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black"
                 :to="{ name: 'claims-id', params: { id: claim.entity_id } }"
      >{{ claim.name }} : {{ claim.entity_id }}
      </nuxt-link>
      <v-btn icon density="compact" v-if="shouldShowMoreMembers" @click="toggleShowMoreMembers">
        <v-icon>{{ showMoreMembers ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
      </v-btn>
    </v-card-title>
    <v-card-text :class="computedClass">
      <v-table :class="computedClass" density="compact">
        <tbody>
        <tr style='text-align: right'>
          <th>Owner:</th>
          <td>{{ claim.owner_player_entity_id }}</td>
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
            <template v-for="(member, index) of members" :key="index">
              <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black"
                         :to="{ name: 'players-id', params: { id: member.entity_id } }"
              >{{ member.user_name }}
              </nuxt-link>
              {{
                index + 1 < members.length ? ', ' : ''
              }}{{ shouldShowMoreMembers && index + 1 === defaultMembers && !showMoreMembers ? '...' : '' }}
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