<script setup lang="ts">
export interface CardClaimProps {
  claim: any;
  defaultMembers?: number;
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

const claimOwner = computed(() => {
  if (claim === undefined) {
    return "";
  }

  return (
    claim.members.find(
      (member) => member.entity_id === claim.owner_player_entity_id,
    )?.user_name ?? `No owner`
  );
});
</script>

<template>
  <v-card>
    <v-card-item>
      <v-card-title>
        <nuxt-link
          class="text-decoration-none text-high-emphasis font-weight-black"
          :to="{ name: 'claims-id', params: { id: claim.entity_id } }"
        >
          {{ claim.name }}
        </nuxt-link>
      </v-card-title>
      <template #append v-if="shouldShowMoreMembers">
        <v-btn icon density="compact" @click="toggleShowMoreMembers">
          <v-icon>{{ showMoreMembers ? 'mdi-chevron-up' : 'mdi-chevron-down' }}</v-icon>
        </v-btn>
      </template>
    </v-card-item>
    <v-card-text :class="computedClass">
      <v-table :class="computedClass" density="compact">
        <tbody>
        <tr style='text-align: right'>
          <th>Owner:</th>
          <td>{{ claimOwner }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Supplies:</th>
          <td>{{ parseInt(claim.supplies) }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Tiles:</th>
          <td>{{ claim.num_tiles }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Location:</th>
          <td>
            N: {{ Math.ceil(claim.location[0][1] / 3)  }}, E: {{ Math.ceil(claim.location[0][0] / 3) }}
          </td>
        </tr>
        <tr v-if="claim.tier" style='text-align: right'>
          <th>Tier:</th>
          <td>
            {{ claim.tier }}
          </td>
        </tr>
        <tr v-if="claim.running_upgrade" style='text-align: right'>
          <th>Research:</th>
          <td>
            {{ claim.running_upgrade.description }}
          </td>
        </tr>
        <tr style='text-align: right'>
          <th>Members ({{ claim.members.length }}):</th>
          <td v-if="claim.members.length > 0">
            <template v-for="(member, index) of members" :key="index">
              <nuxt-link class="text-decoration-none text-high-emphasis font-weight-black"
                         :to="{ name: 'players-id', params: { id: member.entity_id } }"
              >{{ member.user_name }}{{
                  index + 1 < members.length ? ', ' : ''
                }}
              </nuxt-link>
              {{ shouldShowMoreMembers && index + 1 === defaultMembers && !showMoreMembers ? '...' : '' }}
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