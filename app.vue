<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
onBeforeMount(() => {
  const mq = window.matchMedia('(prefers-color-scheme: dark)')

  const theme = useTheme()

  theme.global.name.value = mq.matches ? 'dark' : 'light'

  mq.addEventListener('change', (e) => {
    theme.global.name.value = theme.global.current.value.dark ? 'light' : 'dark'
  })
})

</script>

