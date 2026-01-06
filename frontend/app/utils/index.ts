export const levelToColor = (level: number) => {
  const theme = useTheme();
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-4";
  }

  if (1 <= level && level <= 19) {
    return `grey${colorEffect}`;
  }
  if (20 <= level && level <= 29) {
    return `orange${colorEffect}`;
  }
  if (30 <= level && level <= 39) {
    return `green${colorEffect}`;
  }
  if (40 <= level && level <= 49) {
    return `blue${colorEffect}`;
  }
  if (50 <= level && level <= 59) {
    return `purple${colorEffect}`;
  }
  if (60 <= level && level <= 69) {
    return `red${colorEffect}`;
  }
  if (70 <= level && level <= 79) {
    return `yellow${colorEffect}`;
  }
  if (80 <= level && level <= 89) {
    return `cyan${colorEffect}`;
  }
  if (90 <= level && level <= 99) {
    return `blue-grey${colorEffect}`;
  }
  if (100 <= level) {
    return `lime-accent-4`;
  }
};

export const getTierColor = (tier: number) => {
  const theme = useTheme();
  const colorEffect = theme.global.current.value.dark ? "" : "-darken-4";
  const colors: Record<number, string> = {
    1: `grey${colorEffect}`,
    2: `orange${colorEffect}`,
    3: `green${colorEffect}`,
    4: `blue${colorEffect}`,
    5: `purple${colorEffect}`,
    6: `red${colorEffect}`,
    7: `yellow${colorEffect}`,
    8: `cyan${colorEffect}`,
    9: `blue-grey${colorEffect}`,
    10: `lime-accent-4`,
  };
  return colors[tier] || `grey${colorEffect}`;
};
