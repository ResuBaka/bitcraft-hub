<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { ClaimResponse } from "~/types/ClaimResponse";

const emit = defineEmits({
  search_changed(payload: string) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
  model_changed(payload: bigint) {
    // return `true` or `false` to indicate
    // validation pass / fail
  },
});
const claim = ref<string>("");
const claim_id = ref<string | null>(null);
const router = useRouter();

const {
  data: claimData,
  pending,
  refresh: refreshClaim,
} = await useLazyFetchMsPack<ClaimResponse>(
  () => {
    return `/api/bitcraft/claims`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};

      if (claim.value) {
        options.query.search = claim.value;
      }
      options.query.per_page = 20;

      if (Object.keys(options.query).length >= 2) {
        const query = { claim: claim.value };
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },
  },
);

watchThrottled(
  () => [claim.value],
  () => {
    refreshClaim();
  },
  { throttle: 50 },
);

const claimOptions = computed(() => {
  return (claimData.value?.claims ?? []).map((item) => ({
    label: item.name,
    value: item.entity_id.toString(),
  }));
});

watch(claim, (value) => {
  emit("search_changed", value);
});

watch(claim_id, (value) => {
  if (!value) {
    return;
  }

  emit("model_changed", BigInt(value));
});
</script>

<template>
  <UInputMenu
    v-model="claim_id"
    v-model:search-term="claim"
    :items="claimOptions"
    value-key="value"
    label-key="label"
    icon="i-lucide-search"
    placeholder="Search or select a claim"
    :loading="pending"
    clear
  />
</template>
