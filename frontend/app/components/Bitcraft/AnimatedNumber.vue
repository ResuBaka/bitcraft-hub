<script setup>
const props = defineProps({
  value: {
    type: Number,
    default: 0,
  },
  animate: {
    type: Boolean,
    default: true,
  },
  speed: {
    // smaller is faster
    type: Number,
    default: 5,
  },
  formater: {
    type: Function,
    default: (val) => val,
  },
  color: {
    type: Boolean,
    default: false,
  },
});

const displayNumber = ref(props.value);
let interval = null;
let timeout = null;
let increase = ref(false);

watch(
  () => [props.value, props.animate],
  ([newVal, animate]) => {
    clearInterval(interval);
    clearTimeout(timeout);

    if (!animate) {
      displayNumber.value = newVal;
      increase.value = false;
      return;
    }

    if (newVal === displayNumber.value) {
      return;
    }

    interval = setInterval(() => {
      if (Math.floor(displayNumber.value) !== Math.floor(newVal)) {
        let change = (newVal - displayNumber.value) / props.speed;
        change = change >= 0 ? Math.ceil(change) : Math.floor(change);
        displayNumber.value = displayNumber.value + change;
        increase.value = change > 0;
      } else {
        displayNumber.value = newVal;
        timeout = setTimeout(() => {
          increase.value = false;
        }, 3000);
        clearInterval(interval);
      }
    }, 20);
  },
);

onBeforeUnmount(() => {
  clearInterval(interval);
  clearTimeout(timeout);
});
</script>

<template>
  <span
    :class="[
      'transition-colors duration-300',
      increase ? 'text-emerald-600 dark:text-emerald-400' : '',
    ]"
  >
    {{ formater ? formater(displayNumber) : displayNumber }}
  </span>
</template>
