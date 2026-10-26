<script setup lang="ts">
import { useStorage, useWindowScroll, useWindowSize } from "@vueuse/core";

const snapPositions = {
  "top-left": [0, 0],
  "top-center": [1, 0],
  "top-right": [2, 0],
  "center-left": [0, 1],
  "center-right": [2, 1],
  "bottom-left": [0, 2],
  "bottom-center": [1, 2],
  "bottom-right": [2, 2],
} as const;
type SnapPosition = keyof typeof snapPositions;

const corner = useStorage<SnapPosition>("back-to-top-corner", "bottom-right", undefined, {
  initOnMounted: true,
});
const { y } = useWindowScroll();
const { width, height } = useWindowSize();
const drag = ref<{
  pointerId: number;
  startX: number;
  startY: number;
  left: number;
  top: number;
  size: number;
} | null>(null);
const position = ref<{ left: number; top: number } | null>(null);
let suppressClick = false;
const selectedPosition = computed(() =>
  Object.hasOwn(snapPositions, corner.value) ? corner.value : "bottom-right",
);

const placement = computed(() => {
  if (position.value) {
    return { left: `${position.value.left}px`, top: `${position.value.top}px` };
  }

  const [x, y] = snapPositions[selectedPosition.value];
  const vertical = y === 0 ? "top" : "bottom";
  const horizontal = x === 0 ? "left" : "right";

  return {
    [y === 1 ? "top" : vertical]: y === 1 ? "50%" : `max(1.5rem, env(safe-area-inset-${vertical}))`,
    [x === 1 ? "left" : horizontal]:
      x === 1 ? "50%" : `max(1.5rem, env(safe-area-inset-${horizontal}))`,
    translate: `${x === 1 ? "-50%" : "0"} ${y === 1 ? "-50%" : "0"}`,
  };
});

function startDrag(event: PointerEvent) {
  if (!event.isPrimary || event.button !== 0) return;

  const button = event.currentTarget as HTMLButtonElement;
  const rect = button.getBoundingClientRect();
  suppressClick = false;
  drag.value = {
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    left: rect.left,
    top: rect.top,
    size: rect.width,
  };
  button.setPointerCapture(event.pointerId);
}

function moveDrag(event: PointerEvent) {
  const current = drag.value;
  if (!current || current.pointerId !== event.pointerId) return;

  const dx = event.clientX - current.startX;
  const dy = event.clientY - current.startY;
  if (!position.value && Math.hypot(dx, dy) < 6) return;

  suppressClick = true;
  position.value = {
    left: Math.max(0, Math.min(width.value - current.size, current.left + dx)),
    top: Math.max(0, Math.min(height.value - current.size, current.top + dy)),
  };
}

function endDrag(event: PointerEvent) {
  if (!drag.value || drag.value.pointerId !== event.pointerId) return;

  if (position.value) {
    const centerX = position.value.left + drag.value.size / 2;
    const centerY = position.value.top + drag.value.size / 2;
    const inset = 24 + drag.value.size / 2;
    let nearest: SnapPosition = selectedPosition.value;
    let nearestDistance = Infinity;

    for (const name of Object.keys(snapPositions) as SnapPosition[]) {
      const [x, y] = snapPositions[name];
      const targetX = x === 1 ? width.value / 2 : x === 0 ? inset : width.value - inset;
      const targetY = y === 1 ? height.value / 2 : y === 0 ? inset : height.value - inset;
      const distance = Math.hypot(centerX - targetX, centerY - targetY);
      if (distance < nearestDistance) {
        nearest = name;
        nearestDistance = distance;
      }
    }
    corner.value = nearest;
  }

  position.value = null;
  drag.value = null;
}

function scrollToTop(event: MouseEvent) {
  if (suppressClick && event.detail !== 0) return;

  window.scrollTo({
    top: 0,
    behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth",
  });
}

function moveWithKeyboard(event: KeyboardEvent) {
  if (!event.altKey || !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(event.key))
    return;

  event.preventDefault();
  const [x, y] = snapPositions[selectedPosition.value];
  const dx = event.key === "ArrowLeft" ? -1 : event.key === "ArrowRight" ? 1 : 0;
  const dy = event.key === "ArrowUp" ? -1 : event.key === "ArrowDown" ? 1 : 0;
  let nextX = x + dx;
  let nextY = y + dy;
  // Skip the page center: only corners and edge centers are snap targets.
  if (nextX === 1 && nextY === 1) {
    nextX += dx;
    nextY += dy;
  }
  const next = (Object.keys(snapPositions) as SnapPosition[]).find(
    (name) => snapPositions[name][0] === nextX && snapPositions[name][1] === nextY,
  );
  if (next) corner.value = next;
}
</script>

<template>
  <Transition name="back-to-top">
    <button
      v-if="y > 300 || drag"
      type="button"
      aria-label="Back to top"
      title="Back to top. Drag to a corner or the center of an edge, or use Alt + arrow keys to reposition."
      class="fixed z-50 flex size-12 touch-none select-none items-center justify-center rounded-full bg-primary text-inverted shadow-lg hover:bg-primary/90 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-primary"
      :class="position ? 'cursor-grabbing' : 'cursor-pointer'"
      :style="placement"
      @pointerdown="startDrag"
      @pointermove="moveDrag"
      @pointerup="endDrag"
      @pointercancel="endDrag"
      @lostpointercapture="endDrag"
      @keydown="moveWithKeyboard"
      @click="scrollToTop"
    >
      <UIcon name="i-lucide-arrow-up" class="size-6" />
    </button>
  </Transition>
</template>

<style scoped>
.back-to-top-enter-active,
.back-to-top-leave-active {
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.back-to-top-enter-from,
.back-to-top-leave-to {
  opacity: 0;
  transform: scale(0.8);
}

.back-to-top-leave-active {
  pointer-events: none;
}

@media (prefers-reduced-motion: reduce) {
  .back-to-top-enter-active,
  .back-to-top-leave-active {
    transition: none;
  }
}
</style>
