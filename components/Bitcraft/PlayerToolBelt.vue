<script setup lang="ts">
const props = defineProps({
  inventory: Object,
})

const theme = useTheme();


const tierColor = computed(() => {
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  const colors = {
    1: `grey${colorEffect}`,
    2: `green${colorEffect}`,
    3: `blue${colorEffect}`,
    4: `purple${colorEffect}`,
    5: `yellow${colorEffect}`,
    6: `pink${colorEffect}`,
  };

  return colors;
});
</script>

<template>
  <v-card>
    <v-card-title>Tool Belt</v-card-title>
    <v-card-text>
      <v-row>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Axe</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[0].contents.item.tier]}`">{{ inventory.pockets[0].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Saw</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[1].contents.item.tier]}`">{{ inventory.pockets[1].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Chisel</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[2].contents.item.tier]}`">{{ inventory.pockets[2].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Pickaxe</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[3].contents.item.tier]}`">{{ inventory.pockets[3].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Hammer</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[4].contents.item.tier]}`">{{ inventory.pockets[4].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Knife</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[5].contents.item.tier]}`">{{ inventory.pockets[5].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Bow</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[6].contents.item.tier]}`">{{ inventory.pockets[6].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Scissors</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[7].contents.item.tier]}`">{{ inventory.pockets[7].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Hoe</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[8].contents.item.tier]}`">{{ inventory.pockets[8].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Rod</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[9].contents.item.tier]}`">{{ inventory.pockets[9].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Pod</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[10].contents.item.tier]}`">{{ inventory.pockets[10].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Machete</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[11].contents.item.tier]}`">{{ inventory.pockets[11].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Quill</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[12].contents.item.tier]}`">{{ inventory.pockets[12].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Mallet</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[13].contents.item.tier]}`">{{ inventory.pockets[13].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
        <v-col cols=4>
          <v-list-item>
            <v-list-item-title>Mace</v-list-item-title>
            <v-list-item-subtitle :class="`text-${tierColor[inventory.pockets[14].contents.item.tier]}`">{{ inventory.pockets[14].contents.item.name }}</v-list-item-subtitle>
          </v-list-item>
        </v-col>
      </v-row>
    </v-card-text>
  </v-card>
</template>

<style scoped>

</style>