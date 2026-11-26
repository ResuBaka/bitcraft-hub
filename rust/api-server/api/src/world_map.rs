//! Build terrain snapshots out of band; HTTP requests only read finished files.
mod raster;
mod resource_clusters;
pub(super) mod resource_sync;
pub(super) mod resources;

use crate::config::Config;
use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    extract::Path as RoutePath,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use entity::shared::Region;
use game_module::module_bindings::{
    DbConnection, TerrainChunkState, TerrainChunkStateTableAccess, WorldRegionNameStateTableAccess,
    WorldRegionState, WorldRegionStateTableAccess,
};
use spacetimedb_sdk::{DbContext, Event};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::oneshot;

#[derive(Default)]
pub(crate) struct RegionFilter(Option<Vec<Region>>);

impl RegionFilter {
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, StatusCode> {
        let Some(raw) = raw else {
            return Ok(Self::default());
        };
        if raw == "none" {
            return Ok(Self(Some(Vec::new())));
        }
        if raw.len() > 999 {
            return Err(StatusCode::BAD_REQUEST);
        }
        let mut regions = Vec::new();
        for id in raw.split(',') {
            let id = id
                .parse::<Region>()
                .ok()
                .filter(|id| (1..=255).contains(id))
                .ok_or(StatusCode::BAD_REQUEST)?;
            if regions.len() == 255 {
                return Err(StatusCode::BAD_REQUEST);
            }
            regions.push(id);
        }
        regions.sort_unstable();
        regions.dedup();
        Ok(Self(Some(regions)))
    }

    pub(crate) fn includes(&self, region: Region) -> bool {
        self.0
            .as_ref()
            .is_none_or(|regions| regions.binary_search(&region).is_ok())
    }

    pub(crate) fn ids(&self) -> Option<&[Region]> {
        self.0.as_deref()
    }
}

pub(super) fn routes<S>(directory: PathBuf) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let directory = Arc::new(directory);
    let manifest = directory.join("index.json");
    Router::new()
        .route(
            "/world-map/index.json",
            get(move || serve(manifest.clone(), "application/json", "no-cache")),
        )
        .route(
            "/world-map/{version}/{z}/{x}/{filename}",
            get(
                move |RoutePath((version, z, x, filename)): RoutePath<(
                    String,
                    u8,
                    u32,
                    String,
                )>| {
                    let directory = directory.clone();
                    async move {
                        let Some(y) = filename
                            .strip_suffix(".webp")
                            .and_then(|y| y.parse::<u32>().ok())
                        else {
                            return StatusCode::NOT_FOUND.into_response();
                        };
                        if version.len() != 16
                            || !version.bytes().all(|c| c.is_ascii_hexdigit())
                            || z > raster::MAX_ZOOM
                            || x >= (1 << z)
                            || y >= (1 << z)
                        {
                            return StatusCode::NOT_FOUND.into_response();
                        }
                        serve(
                            directory
                                .join(version)
                                .join(z.to_string())
                                .join(x.to_string())
                                .join(format!("{y}.webp")),
                            "image/webp",
                            "public, max-age=31536000, immutable",
                        )
                        .await
                    }
                },
            ),
        )
}

async fn serve(file: PathBuf, mime: &'static str, cache: &'static str) -> Response {
    match tokio::fs::read(&file).await {
        Ok(content) => (
            [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache)],
            content,
        )
            .into_response(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NOT_FOUND.into_response()
        }
        Err(error) => {
            tracing::error!(path = %file.display(), %error, "Could not read atlas asset");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub(super) async fn generate(config: &Config) -> Result<()> {
    let mut world = raster::World::default();
    for database in config.spacetimedb.regional_databases() {
        tracing::info!(database, "Fetching map metadata and terrain");
        let (region, name, rows) = fetch_region(config, database)
            .await
            .with_context(|| format!("Could not fetch map for {database}"))?;
        world.insert(region, name, rows)?;
    }
    let directory = config.world_map.directory.clone();
    tokio::task::spawn_blocking(move || {
        let threads = std::thread::available_parallelism().map_or(1, |count| count.get().min(8));
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()?
            .install(|| publish(world, &directory))
    })
    .await?
}

fn publish(world: raster::World, directory: &Path) -> Result<()> {
    let version = format!("{:016x}", rand::random::<u64>());
    std::fs::create_dir_all(directory)?;
    let root = directory.join(&version);
    std::fs::create_dir(&root)?;
    let temporary_manifest = directory.join(format!("index.{version}.tmp"));
    let result = (|| {
        let manifest = world.build(&root, version)?;
        std::fs::write(&temporary_manifest, serde_json::to_vec(&manifest)?)?;
        std::fs::rename(&temporary_manifest, directory.join("index.json"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_manifest);
        let _ = std::fs::remove_dir_all(&root);
    }
    result
}

async fn fetch_region(
    config: &Config,
    database: &str,
) -> Result<(WorldRegionState, Option<String>, Vec<TerrainChunkState>)> {
    let id: entity::shared::Region = database
        .rsplit('-')
        .next()
        .unwrap_or_default()
        .parse()
        .context("Regional database names must end in a region number")?;
    let token = config.spacetimedb.password.trim();
    let connection = DbConnection::builder()
        .with_uri(config.spacetimedb_url(id))
        .with_database_name(database)
        .with_compression(config.spacetimedb.compression.into())
        .with_token((!token.is_empty()).then_some(token))
        .build()?;
    let locations = Arc::new(Mutex::new(Vec::new()));
    let name = Arc::new(Mutex::new(None));
    let terrain = Arc::new(Mutex::new(Vec::new()));
    let locations_callback = locations.clone();
    connection
        .db
        .world_region_state()
        .on_initial_owned(move |ctx, rows| {
            if matches!(ctx.event, Event::SubscribeApplied) {
                locations_callback.lock().unwrap().extend(rows);
            }
        });
    let name_callback = name.clone();
    connection
        .db
        .world_region_name_state()
        .on_initial_owned(move |ctx, rows| {
            if matches!(ctx.event, Event::SubscribeApplied)
                && let Some(row) = rows.into_iter().find(|row| row.id == 0)
            {
                let value = row.player_facing_name.trim();
                if !value.is_empty() {
                    *name_callback.lock().unwrap() = Some(value.to_owned());
                }
            }
        });
    let terrain_callback = terrain.clone();
    connection
        .db
        .terrain_chunk_state()
        .on_initial_owned(move |ctx, rows| {
            if matches!(ctx.event, Event::SubscribeApplied) {
                terrain_callback.lock().unwrap().extend(rows);
            }
        });
    let (sender, receiver) = oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    let error_sender = sender.clone();
    let _subscription = connection
        .subscription_builder()
        .on_applied(move |_| {
            if let Some(sender) = sender.lock().unwrap().take() {
                let _ = sender.send(Ok(()));
            }
        })
        .on_error(move |_, error| {
            if let Some(sender) = error_sender.lock().unwrap().take() {
                let _ = sender.send(Err(anyhow::anyhow!("{error}")));
            }
        })
        .subscribe([
            "SELECT * FROM world_region_state",
            "SELECT * FROM world_region_name_state",
            "SELECT * FROM terrain_chunk_state WHERE dimension = 1",
        ]);
    let result = tokio::time::timeout(Duration::from_secs(180), async {
        tokio::select! {
            received = receiver => received.context("Map subscription closed before the snapshot")?,
            ended = connection.run_async() => {
                ended?;
                anyhow::bail!("Connection ended before the map snapshot")
            }
        }
    })
    .await
    .context("Map subscription timed out");
    let _ = connection.disconnect();
    result??;
    let mut rows = std::mem::take(&mut *locations.lock().unwrap());
    ensure!(
        rows.len() == 1,
        "Expected one world_region_state row, got {}",
        rows.len()
    );
    let region = rows.pop().unwrap();
    ensure!(
        i32::from(region.region_index) == i32::from(id),
        "World region ID does not match its configured database"
    );
    let chunks = std::mem::take(&mut *terrain.lock().unwrap());
    let name = name.lock().unwrap().take();
    Ok((region, name, chunks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_only_published_assets_with_expected_cache_headers() {
        let root =
            std::env::temp_dir().join(format!("bitcraft-map-http-{:016x}", rand::random::<u64>()));
        let version = "1234567890abcdef";
        let tiles = root.join(version).join("0/0");
        std::fs::create_dir_all(&tiles).unwrap();
        std::fs::write(tiles.join("0.webp"), b"tile").unwrap();
        std::fs::write(root.join("index.json"), b"{}").unwrap();
        assert!(publish(raster::World::default(), &root).is_err());

        let app = routes::<()>(root.clone());
        for (uri, status, cache) in [
            ("/world-map/index.json", StatusCode::OK, Some("no-cache")),
            (
                "/world-map/1234567890abcdef/0/0/0.webp",
                StatusCode::OK,
                Some("public, max-age=31536000, immutable"),
            ),
            (
                "/world-map/1234567890abcdef/0/0/1.webp",
                StatusCode::NOT_FOUND,
                None,
            ),
            (
                "/world-map/1234567890abcdef/0/0/%2e%2e%2fsecret.webp",
                StatusCode::NOT_FOUND,
                None,
            ),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(
                response
                    .headers()
                    .get(header::CACHE_CONTROL)
                    .map(|v| v.to_str().unwrap()),
                cache
            );
        }
        assert_eq!(std::fs::read(root.join("index.json")).unwrap(), b"{}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
