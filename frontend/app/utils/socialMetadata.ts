/** Build absolute, query-free social URLs for a public detail page. */
export function socialUrls(origin: string, configuredSiteUrl: string, path: string, image: string) {
  const base = configuredSiteUrl || origin;
  try {
    const url = new URL(path, base);
    url.search = "";
    url.hash = "";
    const imageUrl = image ? new URL(image, base).toString() : undefined;
    return { url: url.toString(), image: imageUrl };
  } catch {
    return { url: undefined, image: undefined };
  }
}
