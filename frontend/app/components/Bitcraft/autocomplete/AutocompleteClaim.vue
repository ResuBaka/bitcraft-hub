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
const claim = ref<string | undefined>("");
const claim_id = ref<bigint | null | undefined>();
const router = useRouter();

const { data: claimData, refresh: refreshClaim } =
  await useLazyFetchMsPack<ClaimResponse>(
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
  (value, oldValue) => {
    refreshClaim();
  },
  { throttle: 50 },
);
</script>

<template>
                <v-autocomplete
                @update:model-value="(item) => $emit('model_changed', item)"
                @update:search="(item) => $emit('search_changed', item)"
                v-model="claim_id"
                v-model:search="claim"
                :items="claimData?.claims || []"
                item-title="name"
                item-value="entity_id"
                label="claim"
                outlined
                dense
                clearable
            ></v-autocomplete>
</template>