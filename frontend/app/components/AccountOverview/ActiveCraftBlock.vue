<script setup lang="ts">
import { getTierColor } from "~/utils";
import type { PlayerCraft } from "~/types/PlayerCraft";

const props = defineProps<{ playerId: string }>();
const selected = defineModel<string[]>("claims", { required: true });
const { data, pending, error, refresh, liveConnected } = usePlayerCraftSnapshot(
  () => props.playerId,
  "Active",
);
const { seen, acknowledge } = useCraftAcknowledgements(props.playerId, "Active", data);
const activeCrafts = computed(() =>
  (data.value ?? []).filter(
    (craft) =>
      craft.kind === "Active" && (craft.status !== "Complete" || !seen.value.has(craft.id)),
  ),
);
const { options, filtered: crafts } = useCraftClaimFilter(selected, activeCrafts);
const numberFormat = new Intl.NumberFormat();
function progressPercent(craft: PlayerCraft): number | null {
  if (craft.progress == null || craft.progress_total == null) return null;
  const total = Number(craft.progress_total);
  if (!Number.isFinite(total) || total <= 0 || !Number.isFinite(craft.progress)) return null;
  return Math.max(0, Math.min(100, (craft.progress / total) * 100));
}
</script>

<template>
  <section
    class="@container flex min-h-0 min-w-0 flex-col overflow-hidden rounded-lg border border-default p-1.5"
  >
    <div class="flex shrink-0 items-center justify-between gap-1 overflow-x-auto">
      <div class="flex min-w-0 items-center gap-1">
        <slot name="handle" />
        <h3
          class="truncate text-base font-semibold"
          title="Active projects and finished projects you haven't marked as seen"
        >
          Active Crafts
        </h3>
      </div>
      <div class="flex shrink-0 items-center gap-1">
        <span
          :title="liveConnected ? 'WebSocket connected' : 'WebSocket disconnected; use refresh'"
          :aria-label="
            liveConnected ? 'WebSocket connected' : 'WebSocket disconnected; use refresh'
          "
          role="status"
          :class="liveConnected ? 'bg-success' : 'bg-muted'"
          class="size-2 rounded-full"
        />
        <AccountOverviewCraftBlockSettings
          v-model="selected"
          :options="options"
          block-name="Active crafts"
        />
        <UButton
          icon="i-lucide-refresh-cw"
          color="neutral"
          variant="ghost"
          size="xs"
          :loading="pending"
          aria-label="Refresh active crafts"
          @click="refresh()"
        />
        <slot name="actions" />
      </div>
    </div>
    <div class="min-h-0 flex-1 overflow-auto">
      <p v-if="error" class="mt-1 text-sm text-error">Could not load crafts. Refresh to retry.</p>
      <p v-if="pending && !data" class="mt-1 text-sm text-muted">Loading…</p>
      <template v-else-if="data">
        <p v-if="!crafts.length" class="mt-1 text-sm text-muted">
          {{
            selected.length
              ? "No crafts match the selected claims"
              : "No active projects or new completions"
          }}
        </p>
        <ul v-else class="grid grid-cols-1 gap-x-2 gap-y-1 @lg:grid-cols-2 @4xl:grid-cols-3">
          <li
            v-for="craft in crafts"
            :key="craft.id"
            class="flex min-w-0 items-center gap-1.5 text-sm"
          >
            <BitcraftInventoryImg
              v-if="craft.outputs?.length"
              :item="craft.outputs[0]!"
              :width="20"
              :height="20"
              class="shrink-0"
            />
            <div class="min-w-0 flex-1">
              <div class="flex min-w-0 items-baseline gap-1">
                <NuxtLink
                  :to="`/buildings/${craft.building_id}`"
                  class="min-w-0 truncate font-medium hover:underline"
                  :class="
                    craft.outputs?.[0]?.tier != null
                      ? getTierColor(craft.outputs[0].tier)
                      : undefined
                  "
                  :title="`${craft.recipe_name ?? `Recipe #${craft.recipe_id}`} · ${craft.building_name} · Quantity ${numberFormat.format(craft.quantity)}`"
                  >{{ craft.recipe_name ?? `Recipe #${craft.recipe_id}` }}</NuxtLink
                >
                <span v-if="craft.status === 'Complete'" class="shrink-0 text-xs text-success"
                  >Finished</span
                >
                <span v-else-if="craft.status !== 'Active'" class="shrink-0 text-xs text-warning">{{
                  craft.status
                }}</span>
              </div>
              <AccountOverviewCraftCompletionBar
                v-if="craft.status === 'Complete'"
                :value="100"
                :max="100"
                :description="`Mark finished ${craft.recipe_name ?? `recipe #${craft.recipe_id}`} as seen`"
                @acknowledge="acknowledge([craft.id])"
              />
              <progress
                v-else-if="progressPercent(craft) !== null"
                :value="progressPercent(craft)!"
                max="100"
                :aria-label="`Progress for ${craft.recipe_name ?? `recipe ${craft.recipe_id}`}`"
                :title="`Progress: ${numberFormat.format(craft.progress!)} / ${numberFormat.format(craft.progress_total!)}`"
                class="block h-1.5 w-full accent-primary"
              />
              <span v-else class="text-xs text-muted">Progress unknown</span>
            </div>
          </li>
        </ul>
      </template>
    </div>
  </section>
</template>
