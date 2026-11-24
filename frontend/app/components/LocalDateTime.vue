<script setup lang="ts">
defineProps<{ datetime: string | number | Date; seconds?: boolean }>();
</script>

<template>
  <NuxtTime :datetime="datetime" year="numeric" month="short" day="numeric"
    hour="numeric" minute="2-digit" :second="seconds ? '2-digit' : undefined" />
</template>
