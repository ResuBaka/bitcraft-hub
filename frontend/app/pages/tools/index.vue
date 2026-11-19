<script setup lang="ts">
useSeoMeta({
  title: "Tools",
  description: "Explore BitCraft Hub tools.",
});
</script>

<template>
  <UContainer class="py-8">
    <div class="mb-8">
      <h1 class="text-3xl font-bold">Tools</h1>
      <p class="mt-2 text-muted">Explore tools for keeping track of your BitCraft activity.</p>
    </div>

    <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <UCard>
        <div class="flex h-full flex-col items-start gap-4">
          <div>
            <h2 class="text-lg font-semibold">Account Overview</h2>
            <p class="mt-1 text-sm text-muted">
              A place to monitor health, stamina, and active and passive crafts.
            </p>
          </div>
          <UButton
            to="/tools/account-overview"
            label="Open Account Overview"
            trailing-icon="i-lucide-arrow-right"
          />
        </div>
      </UCard>
    </div>
  </UContainer>
</template>
