<template>
  <UApp>
    <div class="flex h-dvh flex-col overflow-hidden">
      <AppHeader class="shrink-0" />
      <main class="relative min-h-0 flex-1">
        <slot />
      </main>
    </div>
  </UApp>
</template>
