// Towers and settlements store small offset tiles. Match BitCraft's
// coord_to_string: convert to axial, scale to the parent tile, then restore
// the row offset. Player movement coordinates use a different precision.
export function empireLocation(x: number, z: number): string {
  const north = Math.round(z / 3);
  const east = Math.round((x - Math.trunc(z / 2)) / 3) + Math.trunc(north / 2);
  return `N: ${north} E: ${east}`;
}

// Game notification arguments already contain large-tile N/E coordinates.
export function formatEmpireNewsArgument(value: string): string {
  return value.replace(/^N:\{0\}, E:\{1\}\|~(-?\d+)\|~(-?\d+)$/, "N: $1 E: $2");
}
