<script setup lang="ts">
import { watchDebounced, watchThrottled } from "@vueuse/shared";
import type { PlayersResponse } from "~/types/PlayersResponse";
import { useDelayedPending } from "~/utils";

const page = ref(1);
const perPage = 6 * 5;

const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");
const showOnlyOnlinePlayers = ref<boolean>(false);

const route = useRoute();
const router = useRouter();

if (route.query.search) {
  debouncedSearch.value = route.query.search as string;
  search.value = route.query.search as string;
}

if (route.query.page) {
  page.value = parseInt(route.query.page as string);
}

const {
  data: players,
  pending,
  refresh,
} = await useLazyFetchMsPack<PlayersResponse>(
  () => {
    const url = `/api/bitcraft/players?`;
    let paramter = new URLSearchParams();

    if (search.value) {
      paramter.set("search", search.value);
    }

    if (page.value) {
      paramter.set("page", page.value.toString());
    }

    if (showOnlyOnlinePlayers.value) {
      paramter.set("online", "true");
    }

    paramter.set("per_page", perPage.toString());

    return url + paramter;
  },
  {
    // onRequest: ({ options }) => {
    //   options.query = options.query || {};
    //
    //   if (search.value) {
    //     options.query.search = search.value;
    //   }
    //
    //   if (page.value) {
    //     options.query.page = page.value;
    //   }
    //
    //   if (perPage) {
    //     options.query.per_page = perPage;
    //   }
    //
    //   if (showOnlyOnlinePlayers.value) {
    //     options.query.online = true;
    //   }
    //
    //   console.log(options.query);
    //
    //   if (Object.keys(options.query).length > 2) {
    //     const query = { ...options.query };
    //     delete query.per_page;
    //     router.push({ query });
    //   } else if (options.query.page <= 1) {
    //     router.push({});
    //   }
    // },
  },
);

const showPending = useDelayedPending(pending, 150);

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

const totalCount = computed(() => {
  if (!players.value?.total) return 0;
  return Number(players.value.total);
});

const length = computed(() => {
  if (!totalCount.value) return 0;
  return Math.ceil(totalCount.value / perPage);
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
  <UContainer class="w-full max-w-none py-8">
    <div class="flex flex-col gap-6">
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1">
          <p class="text-xs uppercase tracking-[0.2em] text-gray-500 dark:text-gray-400">Players</p>
          <div class="flex flex-wrap items-end justify-between gap-3">
            <div>
              <h1 class="text-2xl font-semibold tracking-tight text-gray-900 dark:text-gray-100">
                Player directory
              </h1>
              <p class="text-sm text-gray-600 dark:text-gray-300">
                Search the roster and inspect player activity.
              </p>
            </div>
            <div
              class="flex items-center gap-2 rounded-full border border-gray-200 px-3 py-1 text-xs text-gray-600 shadow-sm dark:border-gray-800 dark:text-gray-300"
            >
              <span>Total</span>
              <span class="font-semibold text-gray-900 dark:text-gray-100">
                {{ totalCount.toLocaleString() }}
              </span>
            </div>
          </div>
        </div>

        <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
          <UInput
            v-model="debouncedSearch"
            icon="i-lucide-search"
            placeholder="Search players"
            variant="outline"
          />
          <div
            class="flex items-center justify-between gap-2 rounded-lg border border-gray-200 px-3 py-2 text-sm text-gray-600 dark:border-gray-800 dark:text-gray-300"
          >
            <div>
              <p class="text-xs uppercase tracking-[0.2em]">Presence</p>
              <p class="text-sm font-medium">Show only online</p>
            </div>
            <USwitch v-model="showOnlyOnlinePlayers" />
          </div>
        </div>
      </div>

      <div class="flex min-h-[44px] justify-center pb-4" :class="showPending ? 'opacity-60' : ''">
        <UPagination
          v-model:page="page"
          :total="totalCount"
          :items-per-page="perPage"
          size="sm"
          :disabled="showPending"
          :sibling-count="4"
          @update:page="changePage"
        />
      </div>

      <UProgress v-if="showPending" color="neutral" />

      <template v-if="currentPlayers.length">
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
          <UCard
            v-for="player in currentPlayers"
            :key="player.entity_id.toString()"
            :ui="{ body: 'p-4' }"
            class="border border-gray-200 dark:border-gray-800"
          >
            <div class="flex items-center justify-between gap-2">
              <NuxtLink
                :to="{ name: 'players-id', params: { id: player.entity_id.toString() } }"
                :class="player.signed_in ? 'text-green-600' : 'text-gray-900 dark:text-gray-100'"
                class="text-sm font-semibold"
              >
                {{ player.username }}
              </NuxtLink>
              <UBadge :color="player.signed_in ? 'success' : 'neutral'" variant="soft">
                {{ player.signed_in ? "Online" : "Offline" }}
              </UBadge>
            </div>
            <div class="mt-3 space-y-2 text-xs text-gray-500 dark:text-gray-400">
              <div class="flex items-center justify-between">
                <span>Played</span>
                <span class="font-medium text-gray-900 dark:text-gray-100">
                  {{ secondsToDaysMinutesSecondsFormat(player.time_played) || "0s" }}
                </span>
              </div>
              <div class="flex items-center justify-between">
                <span>Signed in</span>
                <span class="font-medium text-gray-900 dark:text-gray-100">
                  {{ secondsToDaysMinutesSecondsFormat(player.time_signed_in) || "0s" }}
                </span>
              </div>
            </div>
          </UCard>
        </div>
      </template>
      <UEmpty
        v-else
        icon="i-lucide-users"
        title="No players found"
        description="Try adjusting your search or filters."
      />
    </div>
  </UContainer>
</template>
