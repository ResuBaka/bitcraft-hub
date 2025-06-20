<template>
  <NuxtLayout>
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
const mq = window.matchMedia("(prefers-color-scheme: dark)");
const html = document.documentElement;
const configStore = useConfigStore();
const theme = useTheme();

const themeSwitch = (e) => {
  if (configStore.theme !== "system") return;

  const theme_value = theme.global.current.value.dark ? "light" : "dark";
  html.setAttribute("data-theme", theme_value);

  theme.global.name.value = theme_value;
};
onBeforeMount(() => {
  if (configStore.theme === "dark") {
    theme.global.name.value = "dark";
    html.setAttribute("data-theme", "dark");
  } else if (configStore.theme === "light") {
    theme.global.name.value = "light";
    html.setAttribute("data-theme", "light");
  } else if (configStore.theme === "system") {
    const theme_value = mq.matches ? "dark" : "light";

    html.setAttribute("data-theme", theme_value);

    theme.global.name.value = theme_value;
    mq.addEventListener("change", themeSwitch);
  }
});

watch(
  () => configStore.theme,
  (newValue) => {
    if (newValue === "dark") {
      theme.global.name.value = "dark";
      html.setAttribute("data-theme", "dark");
    } else if (newValue === "light") {
      theme.global.name.value = "light";
      html.setAttribute("data-theme", "light");
    } else if (newValue === "system") {
      const theme_value = mq.matches ? "dark" : "light";

      html.setAttribute("data-theme", theme_value);

      theme.global.name.value = theme_value;
      mq.addEventListener("change", themeSwitch);
    }
  },
);
</script>

