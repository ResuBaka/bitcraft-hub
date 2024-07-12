<script setup lang="ts">
export interface CardClaimProps {
  empire: any;
  defaultMembers?: number;
}

const showMoreMembers = ref(false);

const { empire, defaultMembers } = withDefaults(defineProps<CardClaimProps>(), {
  defaultMembers: 10,
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
    <v-card-item>
      <v-card-title>
        <nuxt-link
          class="text-decoration-none text-high-emphasis font-weight-black"
          :to="{ name: 'empires-id', params: { id: empire.entity_id } }"
        >
          {{ empire.name }}
        </nuxt-link>
      </v-card-title>
    </v-card-item>
    <v-card-text :class="computedClass">
      <v-table :class="computedClass" density="compact">
        <tbody>
          <tr style='text-align: right'>
          <th>Number of claims:</th>
          <td>{{ empire.num_claims }}</td>
        </tr>
        <tr style='text-align: right'>
          <th>Message:</th>
          <td>{{ empire.directive_message }}</td>
        </tr>
        </tbody>
      </v-table>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>