<script setup lang="ts">
const props = defineProps({
  inventory: Object,
});

const theme = useTheme();

const tierColor = computed(() => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  const colors = {
    1: `grey${colorEffect}`,
    2: `orange${colorEffect}`,
    3: `green${colorEffect}`,
    4: `blue${colorEffect}`,
    5: `purple${colorEffect}`,
    6: `red${colorEffect}`,
    7: `yellow${colorEffect}`,
    8: `cyan${colorEffect}`,
    9: `deepPurple${colorEffect}`,
    10: `deepPurple${colorEffect}`,
  };

  return colors;
});

const header = [
  "Axe",
  "Saw",
  "Chisel",
  "Pickaxe",
  "Hammer",
  "Knife",
  "Bow",
  "Scissors",
  "Hoe",
  "Rod",
  "Pod",
  "Machete",
  "Quill",
  "Mallet",
  "Mace",
];

const pockets = computed(() => {
  let index = 0;

  return (
    props.inventory?.pockets.map((pocket) => {
      const title = header[index];

      index++;
      if (!pocket.contents) {
        return {
          ...pocket,
          displayName: "Empty",
          title,
          class: "",
        };
      }

      return {
        ...pocket,
        displayName: pocket.contents.item.name,
        class: `text-${tierColor.value[pocket.contents.item.tier]}`,
        title,
      };
    }) ?? []
  );
});
</script>

<template>
  <v-card>
    <v-card-title>Tool Belt</v-card-title>
    <v-card-text>
      <v-row>
        <v-col cols=4 v-for="pocket in pockets" :key="pocket.id">
          <v-list-item>
            <v-list-item-title>{{ pocket.title }}</v-list-item-title>
            <v-list-item-subtitle :class="pocket.class">{{ pocket.displayName }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
      </v-row>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>