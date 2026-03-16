<script setup lang="ts">
import { watchThrottled } from "@vueuse/shared";
import type { PlayersResponse } from "~/types/PlayersResponse";

const player = ref<string>("");
const player_id = ref<string | null>(null);
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

const {
  data: playerData,
  pending,
  refresh: refreshPlayer,
} = await useLazyFetchMsPack<PlayersResponse>(
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
  () => {
    refreshPlayer();
  },
  { throttle: 50 },
);

const playerOptions = computed(() => {
  return (playerData.value?.players ?? []).map((item) => ({
    label: item.username,
    value: item.entity_id.toString(),
  }));
});

watch(player, (value) => {
  emit("search_changed", value);
});

watch(player_id, (value) => {
  if (!value) {
    return;
  }

  emit("model_changed", BigInt(value));
});
</script>

<template>
  <UInputMenu
    v-model="player_id"
    v-model:search-term="player"
    :items="playerOptions"
    value-key="value"
    label-key="label"
    icon="i-lucide-search"
    placeholder="Search or select a player"
    :loading="pending"
    clear
  />
</template>
