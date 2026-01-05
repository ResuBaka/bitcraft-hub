export const useTierColor = () => {
  const theme = useTheme();
  let colorEffect = "";

  if (theme.global.current.value.dark) {
  } else {
    colorEffect = "-darken-1";
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
    9: `deep-purple${colorEffect}`,
    10: `deep-purple${colorEffect}`,
  };

  return colors;
};
