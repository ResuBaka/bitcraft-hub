<script setup lang="ts">
import { refDebounced } from "@vueuse/core";
import type { ClaimResponse } from "~/types/ClaimResponse";

const props = defineProps<{ options: { value: string; label: string }[]; blockName: string }>();
const selected = defineModel<string[]>({ required: true });
const search = ref("");
const query = refDebounced(search, 250);
const chosenClaim = ref<string | null>(null);
const knownNames = ref<Record<string, string>>({});
const { data, pending, error, refresh } = useLazyFetchMsPack<ClaimResponse>(
  () => `/api/bitcraft/claims?per_page=20&search=${encodeURIComponent(query.value)}`,
  { server: false, immediate: false, watch: false },
);
watch(query, (value) => {
  if (value.trim().length >= 2) void refresh();
});
const searchOptions = computed(() =>
  search.value.trim().length < 2 || search.value !== query.value || pending.value
    ? []
    : (data.value?.claims ?? []).map((claim) => ({
        value: `claim:${String(claim.entity_id)}`,
        label: claim.name,
      })),
);
const visibleOptions = computed(() =>
  props.options.map((option) => ({
    ...option,
    label: knownNames.value[option.value] ?? option.label,
  })),
);
watch(chosenClaim, (value) => {
  if (!value) return;
  const option = searchOptions.value.find((item) => item.value === value);
  if (option) knownNames.value[value] = option.label;
  if (!selected.value.includes(value)) selected.value = [...selected.value, value];
  chosenClaim.value = null;
  search.value = "";
});
</script>

<template>
  <UPopover>
    <UButton
      icon="i-lucide-settings-2"
      color="neutral"
      variant="ghost"
      size="xs"
      :aria-label="`${blockName} settings`"
      :title="`${blockName} settings`"
    />
    <template #content>
      <div class="w-64 space-y-2 p-3 text-sm">
        <p class="font-medium">Claims</p>
        <p class="text-xs text-muted">
          None selected shows all. Search to add a claim, or select a location below.
        </p>
        <UInputMenu
          v-model="chosenClaim"
          v-model:search-term="search"
          :items="searchOptions"
          :loading="pending"
          value-key="value"
          label-key="label"
          placeholder="Search claims"
          aria-label="Search claims"
          ignore-filter
          class="w-full"
        />
        <p v-if="error" class="text-xs text-error">Could not search claims. Try another search.</p>
        <div v-if="visibleOptions.length" class="max-h-40 space-y-1 overflow-auto">
          <label
            v-for="option in visibleOptions"
            :key="option.value"
            class="flex items-center gap-2"
          >
            <input v-model="selected" type="checkbox" :value="option.value" />
            <span class="break-words">{{ option.label }}</span>
          </label>
        </div>
        <p v-else class="text-xs text-muted">No project locations available.</p>
        <UButton
          v-if="selected.length"
          label="Show all"
          size="xs"
          variant="ghost"
          @click="selected = []"
        />
      </div>
    </template>
  </UPopover>
</template>
