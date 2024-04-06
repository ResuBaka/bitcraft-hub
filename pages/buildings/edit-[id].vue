<script setup lang="ts">
const { data } = await useFetch<Building[]>("/api/buildings/");
</script>

<template>

</template>

<style scoped>

</style>