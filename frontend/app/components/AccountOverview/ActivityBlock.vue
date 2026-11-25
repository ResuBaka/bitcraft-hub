<script setup lang="ts">
import { overviewActivityKey } from "~/composables/useOverviewActivity";
import { overviewStatsKey } from "~/composables/useOverviewStats";
import { activityCraft, activityCraftProgress } from "~/utils/playerActivity";
import { IdleAlertTimer } from "~/utils/accountOverviewAlerts";
import { getTierColor } from "~/utils";
import { formatNumber } from "~/utils/formatNumber";

const props = defineProps<{ playerId: string }>();
const websocket = useWebsocketStore();
const { raw, current, snapshot, pending, error, refreshActivity } = inject(overviewActivityKey)!;
const { data: stats } = inject(overviewStatsKey)!;
const {
  data: crafts,
  pending: loadingCrafts,
  refresh: refreshCrafts,
} = usePlayerCraftSnapshot(() => current.value.craftOwnerId ?? props.playerId, "Active");
const project = computed(() => activityCraft(current.value, crafts.value));
const craftProgress = computed(() => activityCraftProgress(project.value));
const resourceHealth = useTemplateRef("resourceHealth");

function refresh() {
  void refreshActivity();
  void resourceHealth.value?.refresh();
  if (current.value.recipeKind === "craft") void refreshCrafts();
}

type RecipeDetails = {
  kind: "craft" | "gather";
  id: number;
  name: string | null;
  tier?: number | null;
  stamina_requirement?: number | null;
  max_health?: number | null;
};
const recipeKey = computed(() =>
  current.value.recipeKind && current.value.recipeId != null
    ? `${current.value.recipeKind}:${current.value.recipeId}`
    : null,
);
const recipes = ref<Record<string, RecipeDetails>>({});
const { data: recipe, refresh: refreshRecipe } = useLazyFetchMsPack<RecipeDetails>(
  () =>
    `/api/bitcraft/activity/recipe/${current.value.recipeKind ?? "craft"}/${current.value.recipeId ?? 0}`,
  { server: false, immediate: false, watch: false, key: `activity-recipe:${props.playerId}` },
);
watch(recipe, (value) => {
  if (value) recipes.value[`${value.kind}:${value.id}`] = value;
});
watch(
  recipeKey,
  (key) => {
    if (key && !(key in recipes.value)) void refreshRecipe();
  },
  { immediate: true },
);
const currentRecipe = computed(() =>
  recipeKey.value ? recipes.value[recipeKey.value] : undefined,
);
const tier = computed(() => project.value?.outputs?.[0]?.tier ?? currentRecipe.value?.tier);
const idleTimer = new IdleAlertTimer();
const notify = useOverviewNotify();
let lastRecipe: string | null = null;
watchEffect(() => {
  const activity = websocket.isConnected ? raw.value : null;
  if (activity?.label !== "Idle") {
    lastRecipe =
      activity?.recipeKind && activity.recipeId != null
        ? `${activity.recipeKind}:${activity.recipeId}`
        : null;
  }
  const cost = lastRecipe ? recipes.value[lastRecipe]?.stamina_requirement : null;
  const event = idleTimer.update(activity, Date.now(), stats.value, cost ?? 0);
  if (event) notify?.(props.playerId, event);
});
</script>

<template>
  <section
    class="@container flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default p-1.5"
  >
    <div class="flex shrink-0 items-center justify-between gap-1 overflow-x-auto">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <h3 class="truncate text-base font-semibold">Activity</h3>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <UButton
          icon="i-lucide-refresh-cw"
          size="xs"
          color="neutral"
          variant="ghost"
          :loading="pending"
          aria-label="Refresh activity"
          @click="refresh"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <p v-if="error && !snapshot" class="mt-1 text-sm text-error">
        Could not load activity. Refresh to retry.
      </p>
      <div v-else class="flex items-start gap-1.5 text-sm">
        <UIcon :name="current.icon" class="mt-0.5 size-4 shrink-0 text-muted" />
        <div class="min-w-0 flex-1">
          <p class="break-words leading-tight">
            <span class="font-medium">{{ current.label }}</span>
            <span v-if="current.detail" class="text-muted"> · {{ current.detail }}</span>
            <template v-if="current.recipeKind">
              <span class="text-muted"> · </span>
              <span :class="tier != null && tier > 0 ? getTierColor(tier) : 'text-muted'">
                {{
                  project?.recipe_name ??
                  currentRecipe?.name ??
                  (current.recipeId == null ? "Recipe unavailable" : `Recipe #${current.recipeId}`)
                }}
              </span>
            </template>
          </p>
          <template v-if="current.recipeKind === 'craft'">
            <NuxtLink
              v-if="project"
              :to="`/buildings/${project.building_id}`"
              class="text-xs text-muted hover:underline"
            >
              {{ project.building_name }} · Quantity {{ formatNumber(project.quantity) }}
            </NuxtLink>
            <template v-if="craftProgress">
              <div class="mt-0.5 flex items-center gap-1.5">
                <progress
                  class="block h-1.5 min-w-0 flex-1 accent-primary"
                  :value="craftProgress.percent"
                  max="100"
                  aria-label="Craft progress"
                />
                <span class="shrink-0 text-xs text-muted"
                  >{{ formatNumber(craftProgress.progress) }} / {{ formatNumber(craftProgress.total) }}</span
                >
              </div>
            </template>
            <p v-else-if="project?.progress != null" class="text-xs text-muted">
              {{ formatNumber(project.progress) }} progress · Total unknown
            </p>
            <p v-else class="text-xs text-muted">
              {{ loadingCrafts ? "Loading active craft…" : "Progress unavailable" }}
            </p>
          </template>
          <AccountOverviewResourceHealth
            v-else-if="current.recipeKind === 'gather' && current.targetId"
            ref="resourceHealth"
            :key="current.targetId"
            :target-id="current.targetId"
            :max-health="currentRecipe?.max_health"
          />
          <p v-else-if="current.recipeKind === 'gather'" class="text-xs text-muted">
            Resource health unavailable
          </p>
        </div>
      </div>
      <p v-if="snapshot && !websocket.isConnected" class="mt-1 text-xs text-muted">
        Snapshot; live updates disconnected
      </p>
    </div>
  </section>
</template>
