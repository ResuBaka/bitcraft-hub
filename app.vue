<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
onBeforeMount(() => {
  const mq = window.matchMedia('(prefers-color-scheme: dark)')
  const themeCookie = useCookie('theme')

  const themeSwitch = (e) => {
    theme.global.name.value = theme.global.current.value.dark ? 'light' : 'dark'
  }
  const theme = useTheme()

  if (themeCookie.value && themeCookie.value === 'dark') {
    theme.global.name.value = 'dark'
  } else if (themeCookie.value && themeCookie.value === 'light') {
    theme.global.name.value = 'light'
    mq.removeEventListener('change', themeSwitch)
  } else {
    theme.global.name.value = mq.matches ? 'dark' : 'light'
    mq.addEventListener('change', themeSwitch)
  }
})
</script>

