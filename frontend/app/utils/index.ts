export const levelToColor = (level: number) => {
  if (1 <= level && level <= 19) {
    return "text-tier-1";
  }
  if (20 <= level && level <= 29) {
    return "text-tier-2";
  }
  if (30 <= level && level <= 39) {
    return "text-tier-3";
  }
  if (40 <= level && level <= 49) {
    return "text-tier-4";
  }
  if (50 <= level && level <= 59) {
    return "text-tier-5";
  }
  if (60 <= level && level <= 69) {
    return "text-tier-6";
  }
  if (70 <= level && level <= 79) {
    return "text-tier-7";
  }
  if (80 <= level && level <= 89) {
    return "text-tier-8";
  }
  if (90 <= level && level <= 99) {
    return "text-tier-9";
  }
  if (100 <= level) {
    return "text-tier-10";
  }
  return "text-tier-1";
};

export const getTierColor = (tier: number) => {
  const colors: Record<number, string> = {
    1: "text-tier-1",
    2: "text-tier-2",
    3: "text-tier-3",
    4: "text-tier-4",
    5: "text-tier-5",
    6: "text-tier-6",
    7: "text-tier-7",
    8: "text-tier-8",
    9: "text-tier-9",
    10: "text-tier-10",
  };
  return colors[tier] || "text-tier-1";
};

export const tierToTextClass = (tier?: number | null) => {
  if (!tier || tier < 1) {
    return "text-tier-1";
  }
  if (tier > 10) {
    return "text-tier-10";
  }
  return `text-tier-${tier}`;
};

export const tierToBackgroundClass = (tier?: number | null) => {
  if (!tier || tier < 1) {
    return "bg-tier-1";
  }
  if (tier > 10) {
    return "bg-tier-10";
  }
  return `bg-tier-${tier}`;
};

export const tierToBorderClass = (tier?: number | null) => {
  if (!tier || tier < 1) {
    return "border-tier-1";
  }
  if (tier > 10) {
    return "border-tier-10";
  }
  return `border-tier-${tier}`;
};

export const tierToBorderClassByLevel = (level: number) => {
  return `border-tier-${level % 10}`;
};

export const rarityToTextClass = (rarity?: string | null) => {
  if (!rarity) {
    return "text-rarity-common";
  }
  return `text-rarity-${rarity.toLowerCase()}`;
};

export const rarityToBorderClass = (rarity?: string | null) => {
  if (!rarity) {
    return "border-rarity-common";
  }
  return `border-rarity-${rarity.toLowerCase()}`;
};

export const useDelayedPending = (pending: Ref<boolean>, delayMs: number) => {
  const showPending = ref(false);
  let pendingTimer: ReturnType<typeof setTimeout> | null = null;

  const clearPendingTimer = () => {
    if (pendingTimer) {
      clearTimeout(pendingTimer);
      pendingTimer = null;
    }
  };

  watch(pending, (isPending) => {
    if (isPending) {
      clearPendingTimer();

      pendingTimer = setTimeout(() => {
        showPending.value = true;
      }, delayMs);
    } else {
      clearPendingTimer();
      showPending.value = false;
    }
  });

  onBeforeUnmount(() => {
    clearPendingTimer();
  });

  return showPending;
};
