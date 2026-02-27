<script setup lang="ts">
const items = computed(() => [
  {
    label: 'Leaderboards',
    to: '/',
  },
  {
    label: 'Items',
    to: '/items',
  },
  {
    label: 'Claims',
    to: '/claims',
  },
  {
    label: 'Players',
    to: '/players',
  },
//   {
//   label: 'Trade orders',
//   to: '/tradeOrders',
// },
  {
    label: 'Buildings',
    to: '/buildings',
  },
])
</script>

<template>
  <UHeader :ui="{
    container: 'max-w-full'
  }">
    <template #left>
      <NuxtLink to="/">
        BitCraft Hub (Under construction 🚧)
      </NuxtLink>

<!--      <TemplateMenu />-->
    </template>

    <template #right>
      <UNavigationMenu
          :items="items"
          variant="link"
          class="hidden lg:block"
      />

      <UButton
          label="Download App"
          variant="subtle"
          class="hidden lg:block"
      />

      <UColorModeSelect />
    </template>

    <template #body>
      <UNavigationMenu
          :items="items"
          orientation="vertical"
          class="-mx-2.5"
      />
      <UButton
          class="mt-4"
          label="Download App"
          variant="subtle"
          block
      />
    </template>
  </UHeader>
</template>