# World Atlas

`/map` shows the current Overworld. When a terrain snapshot is available,
the map uses BitCraft's Cartesian projection (`x` east, `z` north) and requests
only visible WebP tiles. At very close zoom, it reads colors from the finest
tiles and draws crisp large terrain hexes instead of magnifying raster edges.
Without a snapshot, it shows the 5×5 region layout
with the nine active regions highlighted (7–9, 12–14, 17–19). This fallback is
schematic and does not show terrain or claim to know live world coordinates.

The map fills the viewport below the navigation bar. The **Layers** menu toggles
terrain and overlays. Region grid lines, numbers, and names are off by default
and can be enabled in **Settings**. Names require a snapshot that includes them.
Enable **Claims** to see the tier at each claim flag; click a marker to open a
popover with the claim name, member and tile counts, and a link to its page. Markers come from
the API on demand and do not require rebuilding terrain tiles.
Badges shrink as you zoom out without moving off their flags. Higher tiers
stay visible first when badges overlap.

The URL saves the map position, zoom, visible layers, and selected region or
claim. Copy it to share the view. Map updates replace the current URL rather
than adding a browser history entry for every pan or zoom.
The bottom-left readout shows the map center in in-game **N / E** coordinates,
using the same units as player pages.

On HTTPS or localhost, recent paving, claim-detail, and resource responses
are cached in the browser. Revisiting the same view can show the saved overlay
while the API refreshes it. Terrain uses the normal tile cache, and online
player positions are never saved.

Use **Settings → Region data** to choose which regions load overlays and live
player data. Terrain and region labels stay visible. The nine main regions and
16 event regions each have their own enable-all and disable-all controls.
You can also switch individual resource markers
and their legend from icons to colored dots. These preferences stay in your
browser and never change shared map links.

Under **Settings → Map display**, use **Icon size** for markers on the map only
and **Info size** for region labels, coordinates, legend text, and
claim details. Both sliders range from 75% to 175% and reset independently to
100%. These preferences are saved only in the browser and do not change the camera.

The **Claims** row opens a side menu for claim borders, claim banks, regional
banks, markets, waystones, and tier markers. Its checkbox remains the master
switch. Turning it off preserves the individual choices. Building markers
appear at regional zoom, while borders still require a close-up. Both refresh
for the visible area while panning or zooming; tier filters apply at every zoom.
These choices are also saved in the URL.

The separate **Buildings** layer shows overworld buildings at close zoom.
Markers include the highest positive level in the building's game description
(`T5`, for example); unlevelled buildings have no tier label. Its flyout can
toggle crafting, refining, trading, housing, storage, and other buildings.
These filters are shareable and do not change the Claims layer's service options.

Enable **Online players** to see teal dots for signed-in characters in the
Overworld. The layer opens a WebSocket only when enabled, updates from the live
game feed, and reconnects automatically. Interior locations and signed-out
players are not shown.

Open **Monitor** to search for and track players or claims. Tracking a claim
includes its current members; the roster refreshes periodically while the map is
open. Tracked players are highlighted even when the general **Online players**
layer is off. Use **Locate** to center on a visible player. Tracking and favorites
are saved separately in this browser and are never added to shared map links.

Enable **Paving / roads** and zoom in to see the Overworld's standalone paved
hexes. Paving uses Tier 1–10 colors based on the game's paving descriptions,
with clay-orange, olive, and blue-gray tones matched to in-game paving.
If a tile's tier is unavailable, it keeps a stable color based on
its type ID. The overlay refreshes without rebuilding terrain.
When a dense view exceeds the exact-tile limit, the atlas shows simplified
paving across the whole viewport. Zooming in requests individual tiles again.
The game table does not distinguish standalone blueprint paving from pavement
laid by a player.

Use **Resources** to search node types by name, category, or tier and select
up to eight at a time. Wide views use precomputed resource clusters from
PostgreSQL. Counts include all selected types within each cluster, and the
badge uses the most common selected type's color. Badges sit at the average
position of their nodes, and nearby badges of the same color merge across
bucket boundaries. Click a cluster to zoom into that area; at close zoom the
map shows individual nodes with their in-game icons. The resource picker and
the legend below **Layers** use the same sprites. Sprites are
served from `NUXT_PUBLIC_ICON_DOMAIN`, like the rest of the site. A colored
dot is used if a sprite is unavailable. Dense views
show up to 3,000 nodes, nearest the center first, with a hint in the legend.
Zoom in to fetch a smaller area. Your selection is saved in the map URL.
Use the swatch next to a selected resource to set or reset its hex color.
Colors apply to dots and cluster badges and are saved only in this browser.
Use the star next to a resource in the Layers picker to save it to **Monitor →
Favorites**. Use **Show** there to enable the resource overlay and select it.
Saving a favorite alone does not select the resource or change the shared link.

With API live updates enabled, a dedicated SpacetimeDB subscription for each
configured region syncs Overworld resources and their locations. Initial
snapshots are loaded one region at a time, then changes are written to
PostgreSQL in short batches. The last saved data stays available during
reconnects. This does not use SpacetimeDB's HTTP SQL endpoint. On read-only API
replicas that share PostgreSQL, set `[world_map].resource_sync_enabled = false`
to avoid opening duplicate subscriptions.

## Build the current terrain cache

Configure `[spacetimedb]` as for the API: `domain`, `databases` (the regional
modules), and `password` if a token is needed. Use `[world_map]` to choose where
the API writes and reads the generated cache:

```toml
[world_map]
directory = "world-map"
```

From `rust/api-server`, run:

```sh
cargo run --release -- generate-world-map
```

This one-shot command fetches `world_region_state`, `world_region_name_state`,
and dimension-1 `terrain_chunk_state` from each configured regional module, then renders the
terrain into a sparse WebP pyramid. It does not need PostgreSQL. Run it as a
separate job when terrain needs refreshing, not on every API restart. A failed
fetch or render leaves the last published atlas in place.

With the updated API image, you can also run the generator inside the API
container, provided its terrain directory is mounted writable:

```sh
docker exec -it crafting-app-live-1 generate-world-map
```

If your API uses a custom config path, pass `--config-path` to the command.

Terrain tiles and the map backdrop share the same ocean color. The generator
feathers water where terrain coverage ends, without fading land or borders
between regions that both have terrain.

The atlas palette approximates the revealed in-game map: muted olive greens,
earthy browns, and slate-blue water, with subtle relief shading. It does not
apply fog of war. Palette changes take effect when the terrain cache is rebuilt.

The regular API (`serve`) exposes `/world-map/index.json`,
`/world-map/{version}/{z}/{x}/{y}.webp`, and `/world-map/claims` for the separate
claim overlay. In a container, mount the same cache
directory for the generator and API. The manifest is revalidated; tiles have
immutable URLs. Prior versions remain readable for browsers with an older
manifest; prune them separately when they are no longer needed.

Nuxt uses `NUXT_PUBLIC_API_BASE` to find these endpoints. If the manifest is
absent, the map falls back to the schematic.
