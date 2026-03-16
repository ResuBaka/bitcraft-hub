<script setup lang="ts">
defineOptions({ name: "ItemStack" });

const props = defineProps<{
  items: any;
}>();
</script>

<template>
  <div class="flex flex-col gap-3">
    <div
      v-for="(options, optionIndex) of items"
      :key="`option-${optionIndex}`"
      class="flex flex-col gap-2"
    >
      <UCard
        v-for="(item, itemIndex) of options"
        :key="`item-${optionIndex}-${itemIndex}-${item.item_id}`"
        :ui="{ body: 'p-3 sm:p-4' }"
      >
        <div class="flex items-center justify-between gap-3">
          <p class="text-sm text-gray-700 dark:text-gray-200">Item {{ item.item_id }}</p>
          <UBadge color="neutral" variant="soft">Quantity {{ item.quantity }}</UBadge>
        </div>

        <ItemStack v-if="item.inner !== undefined" :items="item.inner" class="mt-3" />
      </UCard>
    </div>
  </div>
</template>
