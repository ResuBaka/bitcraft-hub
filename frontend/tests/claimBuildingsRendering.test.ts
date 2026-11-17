import { describe, expect, test } from "bun:test";
import { compile, createSSRApp, defineComponent, h } from "vue";
import { renderToString } from "vue/server-renderer";

const source = await Bun.file(new URL("../app/components/Bitcraft/Claim/ClaimTabBuildings.vue", import.meta.url)).text();
const card = source.match(/<NuxtLink\s[\s\S]*?<\/NuxtLink>/)?.[0];
if (!card) throw new Error("Building card template not found");
const render = compile(card);

async function renderCard(iconDomain: string | undefined, icon: string | undefined) {
  const app = createSSRApp(defineComponent({
    setup: () => ({
      iconDomain,
      getBuildingIcon: () => icon,
      buildings: [{ entity_id: 42n, building_description_id: 7, building_name: "Smelter" }],
    }),
    render,
  }));
  app.component("NuxtLink", defineComponent({ setup: (_, { slots }) => () => h("a", slots.default?.()) }));
  return renderToString(app);
}

describe("claim building card", () => {
  test("renders the configured building icon without undefined template values", async () => {
    const html = await renderCard("https://icons.example", "smelter");
    expect(html).toContain('src="https://icons.example/smelter.webp"');
    expect(html).toContain("Smelter");
  });
  test("renders the building without an image when its icon or domain is missing", async () => {
    for (const [domain, icon] of [[undefined, "smelter"], ["https://icons.example", undefined]] as const) {
      const html = await renderCard(domain, icon);
      expect(html).toContain("Smelter");
      expect(html).not.toContain("<img");
      expect(html).not.toContain("undefined.webp");
    }
  });
});
