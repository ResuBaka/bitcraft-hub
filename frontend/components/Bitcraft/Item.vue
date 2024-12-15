<script setup lang="ts">
import type { IconAssetUrl } from "~/composables/iconAssetName";

const props = defineProps<{
  item: any;
}>();

const iconUrl = computed<IconAssetUrl>(() => {
  if (!props.item.item.icon_asset_name) {
    return {
      url: "",
      show: false,
    };
  }

  return iconAssetUrlNameAmount(
    props.item.item.icon_asset_name,
    props.item.quantity,
  );
});
</script>

<template>
  <td>
    <template v-if="iconUrl.amount && iconUrl.amount > 1">
      <v-badge class="pt-5" :content="iconUrl.amount">
        <v-img :src="iconUrl.url" height="40" width="40"></v-img>
      </v-badge>
    </template>
    <template v-else>
      <v-img class="pt-5" :src="iconUrl.url" height="50" width="50"></v-img>
    </template>
  </td>
  <td>{{ item.item.name }}</td>
  <td>{{ item.quantity }}</td>
  <td>Item</td>
</template>