<script setup lang="ts">
import { CurveType } from "@unovis/ts";
import type { ItemsAndCargollResponse } from "~/types/ItemsAndCargollResponse";
import type { SnapshotChartData } from "~/types/SnapshotChartData";
const page = ref(1);

const changePage = (val: number) => {
  page.value = val;
};

const tag = ref<string | null>(null);
const tier = ref<number | null>(null);
const search = ref<string | null>("");
const debouncedSearch = ref<string | null>("");

const route = useRoute();

search.value = (route.query.search as string) ?? "";
debouncedSearch.value = search.value;

let debounceTimeout: any = null;
watch(search, (newVal) => {
  if (debounceTimeout) clearTimeout(debounceTimeout);
  debounceTimeout = setTimeout(() => {
    debouncedSearch.value = newVal;
  }, 300);
});

tag.value = (route.query.tag as string) ?? null;

if (route.query.tier) {
  tier.value = parseInt(route.query.tier);
}

const { data: allDesc, pending } = await useLazyFetchMsPack<ItemsAndCargollResponse>(() => {
  return `/api/bitcraft/itemsAndCargo/all`;
});

const { data: snapshotsData, refresh: reloadSnapshots } =
  await useLazyFetchMsPack<SnapshotChartData[]>(() => {
    return `/inventory/stats_snapshots`;
  });

const snapshots = computed(() => snapshotsData.value ?? []);



await reloadSnapshots();

const { data: metaData } = await useLazyFetchMsPack(() => {
  return `/api/bitcraft/itemsAndCargo/meta`;
});

const latestSnapshotItems = computed(() => {
  if (!snapshots.value || snapshots.value.length === 0) return {};
  const last = snapshots.value[snapshots.value.length - 1];
  return last?.items || last || {};
});

const items = computed(() => {
  if (!allDesc.value?.item_desc) {
    return [];
  }
  try {
    const snap = latestSnapshotItems.value as Record<string, number>;
    let vals: any[] = [];
    for (const item of Object.values(allDesc.value.item_desc) as any[]) {
      const qty = snap[item.name];
      if (qty && qty > 0) {
        vals.push([qty, item]);
      }
    }
    
    vals.sort((a, b) => Number(b[0]) - Number(a[0]));

    const searchStr = debouncedSearch.value?.trim().toLowerCase();
    
    if (!searchStr) {
      return vals.slice(0, 19);
    }
    
    const filtered = vals.filter((value: any) => {
      if (Array.isArray(value) && value[1]?.name) {
        const nameLower = value[1].name.toLowerCase();
        if (nameLower.includes(searchStr)) {
          return true;
        }
        if (
          value[1].rarity &&
          value[1].rarity.toLowerCase().includes(searchStr)
        ) {
          return true;
        }
      }
      return false;
    });
    
    return filtered.slice(0, 48);
  } catch (e) {
    return [];
  }
});

const cargo = computed(() => {
  if (!allDesc.value?.cargo_desc) {
    return [];
  }
  try {
    const snap = latestSnapshotItems.value as Record<string, number>;
    let vals: any[] = [];
    for (const item of Object.values(allDesc.value.cargo_desc) as any[]) {
      const qty = snap[item.name];
      if (qty && qty > 0) {
        vals.push([qty, item]);
      }
    }
    
    vals.sort((a, b) => Number(b[0]) - Number(a[0]));

    const searchStr = debouncedSearch.value?.trim().toLowerCase();
    
    if (!searchStr) {
      return vals.slice(0, 19);
    }
    
    const filtered = vals.filter((value: any) => {
      if (Array.isArray(value) && value[1]?.name) {
        const nameLower = value[1].name.toLowerCase();
        if (nameLower.includes(searchStr)) {
          return true;
        }
        if (
          value[1].rarity &&
          value[1].rarity.toLowerCase().includes(searchStr)
        ) {
          return true;
        }
      }
      return false;
    });

    return filtered.slice(0, 48);
  } catch (e) {
    return [];
  }
});

const numberFormat = new Intl.NumberFormat();

useSeoMeta({
  title: () => `Inventory Stats`,
});

const formatTs = (ts: string) => {
  try {
    return new Date(ts).toLocaleString();
  } catch (e) {
    return ts;
  }
};

const sumQuantity = (arr: any) => {
  if (!arr || !Array.isArray(arr)) return 0;
  return arr.reduce((acc: number, x: any) => {
    try {
      if (Array.isArray(x)) return acc + (Number(x[0]) || 0);
      if (typeof x === "object" && x !== null) {
        // try several common shapes
        if ("0" in x) return acc + (Number(x[0]) || 0);
        if ("quantity" in x) return acc + (Number(x.quantity) || 0);
      }
      return acc + (Number(x) || 0);
    } catch (e) {
      return acc;
    }
  }, 0 as number);
};

const LineChartData = computed(() => {
  if (!Array.isArray(snapshots.value) || snapshots.value.length === 0) {
    return [];
  }
  return snapshots.value.filter((s: any) => s && typeof s === "object");
});

const baseCategories = computed(() => {
  const cat: Record<string, any> = {};
  if (!LineChartData.value) return cat;

  const allNames = new Set<string>();
  for (const snapshot of LineChartData.value) {
    for (const key in snapshot) {
      if (key !== "date") {
        allNames.add(key);
      }
    }
  }

  Array.from(allNames).forEach((name) => {
    let hash = 0;
    for (let i = 0; i < name.length; i++) {
      hash = name.charCodeAt(i) + ((hash << 5) - hash);
    }
    const c = (hash & 0x00ffffff).toString(16).toUpperCase();
    const color = "#" + "00000".substring(0, 6 - c.length) + c;

    cat[name] = {
      name,
      color,
      lowerName: name.toLowerCase(),
    };
  });
  return cat;
});

const categories = computed(() => {
  const allCat = baseCategories.value;
  const searchStr = debouncedSearch.value?.trim().toLowerCase();

  let keys = Object.keys(allCat);

  if (searchStr) {
    const exactMatch = keys.find((key) => allCat[key].lowerName === searchStr);
    
    if (exactMatch) {
      keys = [exactMatch];
    } else {
      keys = keys.filter((key) => allCat[key].lowerName.includes(searchStr));
      // Strictly limit the number of categories/lines rendered on the chart so it never hangs
      keys = keys.slice(0, 20);
    }
  } else {
    keys = keys.slice(0, 20);
  }

  const filtered: Record<string, any> = {};
  for (const key of keys) {
    filtered[key] = allCat[key];
  }
  return filtered;
});

const xFormatter = (tick: number): string => {
  return `${LineChartData.value[tick]?.date ?? ""}`;
};
</script>

<template>
  <v-container fluid>
    <v-row>
      <v-col>
        <v-text-field
            v-model="search"
            label="Search"
            outlined
            dense
            clearable
        ></v-text-field>
      </v-col>
<!--      <v-col>-->
<!--        <v-autocomplete-->
<!--            v-model="tag"-->
<!--            :items="metaData?.tags || []"-->
<!--            label="Tag"-->
<!--            outlined-->
<!--            dense-->
<!--            clearable-->
<!--        ></v-autocomplete>-->
<!--      </v-col>-->
<!--      <v-col>-->
<!--        <v-select-->
<!--            v-model="tier"-->
<!--            :items="metaData?.tiers || []"-->
<!--            label="Tier"-->
<!--            outlined-->
<!--            dense-->
<!--            clearable-->
<!--        ></v-select>-->
<!--      </v-col>-->
    </v-row>

    <v-row>
      <v-col>
        <v-progress-linear
            color="yellow-darken-2"
            indeterminate
            :active="pending"
        ></v-progress-linear>
      </v-col>
    </v-row>
    <template v-if="allDesc">
      <v-row class="mb-6">
        <v-col cols="12">
          <v-card>
            <v-card-title>Historical snapshots</v-card-title>
            <v-card-text>
              <div style="height:300px" v-if="snapshots && snapshots.length > 0">
                <LineChart
                  :data="LineChartData"
                  :height="300"
                  :categories="categories"
                  :y-grid-line="true"
                  :x-formatter="xFormatter"
                  :curve-type="CurveType.MonotoneX"
                  :hide-legend="false"
                />
              </div>
              <div v-else style="height:300px; display: flex; align-items: center; justify-content: center; color: #999;">
                No snapshot data available
              </div>
            </v-card-text>
          </v-card>
        </v-col>
      </v-row>
      <v-row>
        <v-col cols="12" md="6" lg="4" xl="3" v-for="item in items" :key="`item:${item[1]?.id}`">
          {{ numberFormat.format(Number(item[0])) }} <b>{{ item[1]?.name }}</b> {{ item[1]?.rarity }} {{ (item[1]?.tier || 0) > 0 ? item[1]?.tier : '' }}
        </v-col>
      </v-row>
      <v-divider class="mt-5 pb-5"  thickness="10" />
      <v-row>
        <v-col cols="12" md="6" lg="4" xl="3" v-for="item in cargo" :key="`cargo:${item[1]?.id}`">
          {{ numberFormat.format(Number(item[0])) }} <b>{{ item[1]?.name }}</b> {{ item[1]?.rarity }} {{ (item[1]?.tier || 0) > 0 ? item[1]?.tier : '' }}
        </v-col>
      </v-row>

    </template>
    <template v-else>
      <v-empty-state
       headline="No items found"
       text="Try changing your search criteria"
      ></v-empty-state>
    </template>
  </v-container>
</template>
