<script setup lang="ts">
import type { EmpireSummary } from "~/types/EmpireSummary";

const props = defineProps<{ empire: EmpireSummary }>();
const initials = computed(() => props.empire.name.trim().split(/\s+/u).slice(0, 2)
  .map(word => Array.from(word)[0]).join("").toUpperCase());
const dye = (hex: string | null, fallback: string) =>
  hex && /^[0-9a-f]{1,6}$/i.test(hex) ? `#${hex.padStart(6, "0")}` : fallback;
</script>

<template>
  <span aria-hidden="true" class="empire-mark" :style="{
    '--field': dye(empire.color, '#52656b'),
    '--trim': dye(empire.accent, '#a2b1ae'),
  }">
    <span>{{ initials }}</span>
  </span>
</template>

<style scoped>
.empire-mark {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 2.8em;
  height: 3.3em;
  padding-bottom: .35em;
  clip-path: polygon(0 0, 100% 0, 100% 76%, 50% 100%, 0 76%);
  background: linear-gradient(135deg, transparent 57%, var(--trim) 57%, var(--trim) 68%, transparent 68%), var(--field);
  color: white;
  font-family: Georgia, serif;
  font-weight: bold;
  text-shadow: 0 1px 4px #000, 0 0 2px #000;
}
</style>
