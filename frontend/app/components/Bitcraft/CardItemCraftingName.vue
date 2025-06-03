<script setup lang="ts">
const props = defineProps<{
  item: any;
  template: string;
  craftId: number;
}>();
const {
  public: { api },
} = useRuntimeConfig();

const { data: neededInCrafting } = useFetchMsPack(() => {
  return `${api.base}/api/bitcraft/recipes/needed_in_crafting/${props.craftId}`;
});
const replacedTempalte = computed(() =>
  props.template
    .replace("{0}", neededInCrafting.value?.name ?? "Unknown")
    .replace("{1}", props.item.name),
);
</script>

<template>
  {{ replacedTempalte }}
</template>

<style scoped>

</style>