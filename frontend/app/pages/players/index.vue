<script setup lang="ts">
import { watchDebounced, watchThrottled } from "@vueuse/shared";
import type { PlayersResponse } from "~/types/PlayersResponse";

const page = ref(1);
const perPage = 6 * 5;

const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");
const showOnlyOnlinePlayers = ref<boolean>(false);

const route = useRoute();
const router = useRouter();

if (route.query.search) {
  debouncedSearch.value = route.query.search as string;
  search.value = debouncedSearch.value;
}

if (route.query.page) {
  page.value = parseInt(route.query.page as string);
}
const {
  public: { api },
} = useRuntimeConfig();

const {
  data: players,
  pending,
  refresh,
} = await useLazyFetchMsPack<PlayersResponse>(
  () => {
    return `${api.base}/api/bitcraft/players`;
  },
  {
    onRequest: ({ options }) => {
      options.query = options.query || {};

      if (search.value) {
        options.query.search = search.value;
      }

      if (page.value) {
        options.query.page = page.value;
      }

      if (perPage) {
        options.query.per_page = perPage;
      }

      if (showOnlyOnlinePlayers.value) {
        options.query.online = true;
      }

      if (Object.keys(options.query).length > 2) {
        const query = { ...options.query };
        delete query.per_page;
        router.push({ query });
      } else if (options.query.page <= 1) {
        router.push({});
      }
    },
  },
);

const changePage = (value: number) => {
  page.value = value;
  router.push({
    query: {
      ...route.query,
      page: value,
    },
  });
  refresh();
};

watchThrottled(
  () => [search.value, showOnlyOnlinePlayers.value],
  (value, oldValue) => {
    if (value[0] !== oldValue[0] || value[1] !== oldValue[1]) {
      page.value = 1;
    }

    refresh();
  },
  { throttle: 50 },
);

watchDebounced(
  debouncedSearch,
  () => {
    if (search.value !== debouncedSearch.value) {
      page.value = 1;
    }

    search.value = debouncedSearch.value;
    refresh();
  },
  { debounce: 100, maxWait: 200 },
);

const currentPlayers = computed(() => {
  return players.value?.players ?? [];
});

const length = computed(() => {
  if (players.value?.total) {
    if (typeof players.value.total == "bigint") {
      return Math.ceil(players.value.total / BigInt(perPage));
    }

    return Math.ceil(players.value?.total / perPage);
  }

  return 0;
});

const theme = useTheme();

const computedClass = computed(() => {
  return {
    "bg-surface-light": theme.global.current.value.dark,
    "bg-grey-lighten-3": !theme.global.current.value.dark,
  };
});

const secondsToDaysMinutesSecondsFormat = (seconds: number) => {
  const days = Math.floor(seconds / (60 * 60 * 24));
  const hours = Math.floor((seconds % (60 * 60 * 24)) / (60 * 60));
  const minutes = Math.floor((seconds % (60 * 60)) / 60);
  const secondsLeft = seconds % 60;

  let result = "";

  if (days > 0) {
    result += `${days}d `;
  }

  if (hours > 0) {
    result += `${hours}h `;
  }

  if (minutes > 0) {
    result += `${minutes}m `;
  }

  if (secondsLeft > 0) {
    result += `${secondsLeft}s`;
  }

  return result;
};

const timeStampToDateSince = (timestamp: number) => {
  const date = new Date(timestamp * 1000);
  const options: Intl.DateTimeFormatOptions = {
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "numeric",
    second: "numeric",
  };
  return date.toLocaleDateString(undefined, options);
};

useSeoMeta({
  title: () => `Players ${players.value?.total ?? 0}`,
  description: "Players",
});
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col cols="10">
        <v-text-field
            v-model="debouncedSearch"
            label="Search"
            outlined
            dense
            clearable
        ></v-text-field>
      </v-col>
      <v-col cols="2">
        <v-checkbox
            v-model="showOnlyOnlinePlayers"
            label="Show only online Player"
        ></v-checkbox>
      </v-col>
    </v-row>
    <v-row>
      <v-col>
        <v-progress-linear
            color="yellow-darken-2"
            indeterminate
            :active="pending"
        ></v-progress-linear>
      </v-col>
    </v-row>
    <v-row>
      <v-col cols="12" md="6" lg="3" xl="2" v-for="player in currentPlayers" :key="player.entity_id.toString()">
        <v-card>
          <template v-slot:title>
            <nuxt-link :class="`text-decoration-none font-weight-black ${player.signed_in ? 'text-green' : 'text-high-emphasis'}`"
                       :to="{ name: 'players-id', params: { id: player.entity_id.toString() } }"
            >{{ player.username }}
            </nuxt-link>
          </template>
          <v-card-text :class="computedClass">
            <v-table :class="computedClass" density="compact">
              <tbody>
              <tr style='text-align: right'>
                <th>Played:</th>
                <td>{{ secondsToDaysMinutesSecondsFormat(player.time_played) }}</td>
              </tr>
              <tr style='text-align: right'>
                <th>Signed in:</th>
                <td>{{ secondsToDaysMinutesSecondsFormat(player.time_signed_in) }}</td>
              </tr>
              </tbody>
            </v-table>
          </v-card-text>
        </v-card>
      </v-col>
      <v-col cols="12">
        <v-pagination
            @update:model-value="changePage"
            v-model="page"
            :length="length"
        ></v-pagination>
      </v-col>
    </v-row>
  </v-container>
</template>
