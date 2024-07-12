<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";
import CardItem from "~/components/Bitcraft/CardItem.vue";
const {
  public: { iconDomain },
} = useRuntimeConfig();

const page = ref(1);
const perPage = 30;

const search = ref<string | null>("");

const route = useRoute();
const router = useRouter();

const tmpPage = (route.query.page as string) ?? null;

if (tmpPage) {
  page.value = parseInt(tmpPage);
}

const { data: empireFetch, pending: claimPnding } = useFetch(() => {
  return `/api/bitcraft/empires/${route.params.id}`;
});

const empire = computed(() => {
  return empireFetch.value ?? undefined;
});

</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col cols="12">
        <v-card height="100%" v-if="empire !== undefined">
          <v-card-item>
            <v-card-title>
              {{ empire.name }}
            </v-card-title>
            <v-card-text>
              <v-row>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>color1_index</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.color1_index }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>color2_index</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.color2_index }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>shard_treasury</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.shard_treasury }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>directive_message</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.directive_message }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>directive_message_timestamp</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.directive_message_timestamp }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>Nobility threshold</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.nobility_threshold }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
                <v-col cols="6" md="6" lg="12">
                  <v-list-item>
                    <v-list-item-title>Number of claims</v-list-item-title>
                    <v-list-item-subtitle>{{ empire.num_claims }}</v-list-item-subtitle>
                  </v-list-item>
                </v-col>
              </v-row>
            </v-card-text>
          </v-card-item>
        </v-card>
      </v-col>
    </v-row>
  </v-container>
</template>
