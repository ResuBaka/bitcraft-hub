<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
const mq = window.matchMedia('(prefers-color-scheme: dark)')
const configStore = useConfigStore()
const theme = useTheme()

const themeSwitch = (e) => {
  if (configStore.theme !== 'system') return

  theme.global.name.value = theme.global.current.value.dark ? 'light' : 'dark'
}
onBeforeMount(() => {
  if (configStore.theme === 'dark') {
    theme.global.name.value = 'dark'
  } else if (configStore.theme === 'light') {
    theme.global.name.value = 'light'
  } else if (configStore.theme === 'system') {
    theme.global.name.value = mq.matches ? 'dark' : 'light'
    mq.addEventListener('change', themeSwitch)
  }
})

watch(() => configStore.theme, (newValue) => {
  if (newValue === 'dark') {
    theme.global.name.value = 'dark'
  } else if (newValue === 'light') {
    theme.global.name.value = 'light'
  } else if (newValue === 'system') {
    theme.global.name.value = mq.matches ? 'dark' : 'light'
    mq.addEventListener('change', themeSwitch)
  }
})
</script>

