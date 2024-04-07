<script setup lang="ts">
const tiers = ref<number[]>([1, 2, 3, 4, 5]);

const { data, refresh, pending } = await useFetch<Profession[]>("/api/professions");

const newProfession = ref({
  id: "",
  icon: ""
})

const valid = ref(false);
const customId = ref(false);

const createProfession = async () => {
  console.log(newProfession.value);

  if (!newProfession.value.id) {
    return;
  }

  console.log(JSON.stringify(newProfession.value, null, 2));

  let temp = await fetch(`/api/professions`, {
    method: "PUT",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(newProfession.value),
  });

  console.log(temp);

  refresh();
};

const spaceRegex = /\s/g;

watch(
    () => newProfession.value.id,
    () => {
      if (!customId.value) {
        newProfession.value.id = newProfession.value.id
            .replace(spaceRegex, "_");
      }
    },
);
</script>

<template>
  <v-container>
    <v-card>
      <v-card-title>New Profession</v-card-title>
      <v-card-text>
        <v-container>
          <v-form v-model="valid">
            <v-row>
              <v-col cols="5">
                <v-text-field v-model="newProfession.id" label="Name"/>
              </v-col>
              <v-col cols="2">
                <v-btn @click="createProfession">Create</v-btn>
              </v-col>
            </v-row>
          </v-form>
        </v-container>
      </v-card-text>
    </v-card>
  </v-container>
</template>
