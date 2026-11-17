<script setup lang="ts">
import { getFavoritePlayerSections, insertPlayerSectionFavorite, movePlayerSectionFavorite, togglePlayerSectionFavorite } from "~/utils/playerSections";

const selected = defineModel<string>({ required: true });
defineProps<{ collapsed?: boolean }>();
const emit = defineEmits<{ select: [] }>();
const config = useConfigStore();
const dragged = ref<string | null>(null);
const dropPreview = ref<{ target: string; position: "before" | "after" } | null>(null);
const sections = [
  { value: "overview", label: "Overview", icon: "i-lucide-user" },
  { value: "deployables", label: "Deployables", icon: "i-lucide-box" },
  { value: "crafting", label: "Crafting projects", icon: "i-lucide-hammer" },
  { value: "xp_history", label: "XP history", icon: "i-lucide-chart-no-axes-combined" },
  { value: "skills", label: "Skills", icon: "i-lucide-award" },
  { value: "traveler_tasks", label: "Traveler tasks", icon: "i-lucide-list-checks" },
  { value: "inventories", label: "Inventories", icon: "i-lucide-backpack" },
  { value: "houses", label: "Accessible houses", icon: "i-lucide-house" },
];
const favorites = computed(() => getFavoritePlayerSections(sections, config.favorite_player_sections));
function select(value: string) {
  selected.value = value;
  emit("select");
}
function toggle(value: string) {
  config.favorite_player_sections = togglePlayerSectionFavorite(config.favorite_player_sections, value);
}
function move(value: string, target: string) {
  config.favorite_player_sections = movePlayerSectionFavorite(favorites.value.map((section) => section.value), value, target);
}
function moveByKey(value: string, direction: -1 | 1) {
  const target = favorites.value[favorites.value.findIndex((section) => section.value === value) + direction];
  if (target) move(value, target.value);
}
function startDrag(event: DragEvent, value: string) {
  dragged.value = value;
  if (event.dataTransfer) {
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", value);
  }
}
function clearDrag() {
  dragged.value = null;
  dropPreview.value = null;
}
function previewAt(element: HTMLElement, clientY: number) {
  const target = element.dataset.favoriteSection;
  if (!dragged.value || !target || target === dragged.value) {
    dropPreview.value = null;
    return;
  }
  const rect = element.getBoundingClientRect();
  dropPreview.value = { target, position: clientY < rect.top + rect.height / 2 ? "before" : "after" };
}
function drop() {
  if (dragged.value && dropPreview.value) {
    config.favorite_player_sections = insertPlayerSectionFavorite(
      favorites.value.map((section) => section.value), dragged.value, dropPreview.value.target, dropPreview.value.position,
    );
  }
  clearDrag();
}
function previewTouch(event: TouchEvent) {
  const touch = event.touches[0];
  const element = touch && document.elementFromPoint(touch.clientX, touch.clientY)
    ?.closest<HTMLElement>("[data-favorite-section]");
  if (touch && element) previewAt(element, touch.clientY);
  else dropPreview.value = null;
}
function endTouch(event: TouchEvent) {
  const touch = event.changedTouches[0];
  const element = touch && document.elementFromPoint(touch.clientX, touch.clientY)
    ?.closest<HTMLElement>("[data-favorite-section]");
  if (touch && element) {
    previewAt(element, touch.clientY);
    drop();
  } else clearDrag();
}
</script>

<template>
  <nav aria-label="Player information" class="flex flex-col gap-1">
    <template v-if="collapsed">
      <template v-if="favorites.length">
        <UIcon name="i-lucide-star" class="mx-auto mt-2 size-4 text-warning" aria-label="Favorites" />
        <UButton v-for="section in favorites" :key="`favorite-${section.value}`"
          :icon="section.icon" :aria-label="`Favorite: ${section.label}`" :title="`Favorite: ${section.label}`"
          :variant="selected === section.value ? 'solid' : 'ghost'" color="neutral"
          class="mx-auto" @click="select(section.value)" />
        <div class="my-1 border-t border-gray-200 dark:border-gray-800" />
      </template>
      <UButton v-for="section in sections" :key="section.value"
        :icon="section.icon" :aria-label="section.label" :title="section.label"
        :aria-current="selected === section.value ? 'page' : undefined"
        :variant="selected === section.value ? 'solid' : 'ghost'" color="neutral"
        class="mx-auto" @click="select(section.value)" />
    </template>
    <template v-else>
    <template v-if="favorites.length">
      <p class="px-2 pt-1 text-xs font-semibold uppercase tracking-wide text-gray-500">Favorites</p>
      <p class="px-2 text-xs text-gray-500">Drag a favorite to reorder, or use the handle with the arrow keys.</p>
      <div v-for="section in favorites" :key="`favorite-${section.value}`" draggable="true" class="relative flex cursor-grab items-center gap-1 active:cursor-grabbing"
        :data-favorite-section="section.value" :class="{ 'opacity-50': dragged === section.value }"
        @dragstart="startDrag($event, section.value)" @dragend="clearDrag"
        @dragover.prevent="previewAt($event.currentTarget as HTMLElement, $event.clientY)"
        @dragleave="dropPreview = null"
        @drop.prevent="previewAt($event.currentTarget as HTMLElement, $event.clientY); drop()">
        <span v-if="dropPreview?.target === section.value" aria-hidden="true"
          class="pointer-events-none absolute inset-x-0 z-10 h-1 rounded-full bg-primary"
          :class="dropPreview.position === 'before' ? '-top-1' : '-bottom-1'" />
        <button type="button"
          class="touch-none cursor-grab rounded p-1 text-gray-500 hover:bg-gray-100 focus-visible:outline focus-visible:outline-2 dark:hover:bg-gray-800"
          :aria-label="`Reorder ${section.label}; use up and down arrow keys`"
          @touchstart="dragged = section.value" @touchmove.prevent="previewTouch" @touchend="endTouch" @touchcancel="clearDrag"
          @keydown.up.prevent="moveByKey(section.value, -1)" @keydown.down.prevent="moveByKey(section.value, 1)">
          <UIcon name="i-lucide-grip-vertical" class="size-4" />
        </button>
        <UButton class="min-w-0 flex-1" :icon="section.icon" :label="section.label"
          :variant="selected === section.value ? 'solid' : 'soft'" color="neutral" @click="select(section.value)" />
        <UButton icon="i-lucide-star" color="warning" variant="soft" size="xs"
          :aria-label="`Unpin ${section.label}`" @click="toggle(section.value)" />
      </div>
    </template>
    <p class="px-2 pt-3 text-xs font-semibold uppercase tracking-wide text-gray-500">All sections</p>
    <div v-for="section in sections" :key="section.value" class="flex items-center gap-1">
      <UButton class="min-w-0 flex-1" :icon="section.icon" :label="section.label"
        :variant="selected === section.value ? 'solid' : 'soft'" color="neutral"
        :aria-current="selected === section.value ? 'page' : undefined" @click="select(section.value)" />
      <UButton icon="i-lucide-star"
        :color="config.favorite_player_sections.includes(section.value) ? 'warning' : 'neutral'"
        :variant="config.favorite_player_sections.includes(section.value) ? 'soft' : 'ghost'"
        :aria-label="`${config.favorite_player_sections.includes(section.value) ? 'Unpin' : 'Pin'} ${section.label}`"
        :aria-pressed="config.favorite_player_sections.includes(section.value)" @click="toggle(section.value)" />
    </div>
    </template>
  </nav>
</template>
