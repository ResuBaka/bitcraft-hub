<script setup lang="ts">
import LeaderboardClaim from "~/components/Bitcraft/LeaderboardClaim.vue";

defineProps<{ claimId?: bigint | number | string }>();
</script>

<template>
  <LeaderboardClaim :claim-id="claimId" />
</template>
