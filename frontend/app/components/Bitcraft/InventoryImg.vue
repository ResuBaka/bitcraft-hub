<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    item: any;
    skipErrorText?: boolean;
  }>(),
  {
    skipErrorText: false,
  },
);

const imagedErrored: boolean = ref(false);
const iconUrl = (item: any) => {
  if (!item?.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameRandom(item.icon_asset_name);
};

const stippedName = computed(() => {
  return props.item.name.replace(/[\[\(\)\{\}\]]/gi, "");
});
</script>

<template>
  <template v-if="!imagedErrored">
    <v-img @error="imagedErrored = true" :src="iconUrl(item).url" height="80" width="80"></v-img>
  </template>
  <template v-else-if="!skipErrorText">
    {{ stippedName.split(" ").map(part => part.charAt(0)).join("") }}
  </template>
</template>

<style scoped>

</style>