<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { PlayersResponse } from "~/types/PlayersResponse";

const player = ref<string | undefined>("");
const player_id = ref<bigint | null | undefined>();
const router = useRouter();

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

const { data: playerData, refresh: refreshPlayer } =
  await useLazyFetchMsPack<PlayersResponse>(
    () => {
      return `/api/bitcraft/players`;
    },
    {
      onRequest: ({ options }) => {
        options.query = options.query || {};

        if (player.value) {
          options.query.search = player.value;
        }
        options.query.per_page = 20;

        if (Object.keys(options.query).length >= 2) {
          const query = { player: player.value };
          router.push({ query });
        } else if (options.query.page <= 1) {
          router.push({});
        }
      },
    },
  );

watchThrottled(
  () => [player.value],
  (value, oldValue) => {
    refreshPlayer();
  },
  { throttle: 50 },
);
</script>

<template>
    <v-autocomplete
                    @update:model-value="(item) => $emit('model_changed', item)"
                    @update:search="(item) => $emit('search_changed', item)"
                    v-model="player_id"
                    v-model:search="player"
                    :items="playerData?.players || []"
                    item-title="username"
                    item-value ="entity_id"
                    label="player"
                    outlined
                    dense
                    clearable
                />
</template>