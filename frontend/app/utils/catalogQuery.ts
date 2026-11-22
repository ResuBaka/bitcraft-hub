export function catalogQueryText(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

export function catalogQueryPage(value: unknown): number {
  const page = Number(catalogQueryText(value));
  return Number.isSafeInteger(page) && page > 0 ? page : 1;
}
