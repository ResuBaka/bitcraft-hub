use super::resource_clusters::{BASE, MAX_LEVEL};
use crate::AppState;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use entity::{resource_desc, shared::Region};
use sea_orm::{DbBackend, EntityTrait, FromQueryResult, QuerySelect, Statement};
use serde::{Deserialize, Serialize};

const LIMIT: usize = 3_000;
const MAX_TYPES: usize = 8;
const MAX_REGIONS: usize = 4;

#[derive(Serialize, FromQueryResult)]
struct ResourceType {
    id: i32,
    name: String,
    tag: String,
    icon_asset_name: String,
    tier: i32,
}

async fn catalog(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    use resource_desc::Column::*;
    let mut rows = resource_desc::Entity::find()
        .select_only()
        .columns([Id, Name, Tag, Tier, IconAssetName])
        .into_model::<ResourceType>()
        .all(&state.conn)
        .await
        .map_err(database_error)?;
    rows.sort_unstable_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.tier.cmp(&b.tier))
            .then(a.id.cmp(&b.id))
    });
    Ok(([(header::CACHE_CONTROL, "public, max-age=300")], Json(rows)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    center_x: Option<i32>,
    center_z: Option<i32>,
    ids: String,
    regions: String,
}

fn parse_list<T: std::str::FromStr + Ord>(input: &str, max: usize) -> Option<Vec<T>> {
    let mut values = Vec::new();
    for entry in input.split(',') {
        if values.len() == max || entry.len() > 11 {
            return None;
        }
        values.push(entry.parse().ok()?);
    }
    values.sort_unstable();
    values.dedup();
    Some(values)
}

impl Params {
    fn center(&self) -> Option<(i32, i32)> {
        match (self.center_x, self.center_z) {
            (None, None) => Some(((self.min_x + self.max_x) / 2, (self.min_z + self.max_z) / 2)),
            (Some(x), Some(z))
                if (self.min_x..=self.max_x).contains(&x)
                    && (self.min_z..=self.max_z).contains(&z) =>
            {
                Some((x, z))
            }
            _ => None,
        }
    }

    fn filters(&self) -> Option<(Vec<i32>, Vec<Region>)> {
        if !(0..=96_000).contains(&self.min_x)
            || !(self.min_x..=96_000).contains(&self.max_x)
            || !(0..=96_000).contains(&self.min_z)
            || !(self.min_z..=96_000).contains(&self.max_z)
        {
            return None;
        }
        let width = self.max_x - self.min_x + 1;
        let height = self.max_z - self.min_z + 1;
        if width > 2048 || height > 2048 || width * height > 1_500_000 {
            return None;
        }
        let regions: Vec<Region> = parse_list(&self.regions, MAX_REGIONS)?;
        if regions.iter().any(|&r| !(1..=255).contains(&r)) {
            return None;
        }
        Some((parse_list(&self.ids, MAX_TYPES)?, regions))
    }
}

#[derive(FromQueryResult)]
struct SyncInfo {
    ready: bool,
}

#[derive(FromQueryResult)]
struct Node {
    x: i32,
    z: i32,
    resource_id: i32,
}

#[derive(Serialize)]
struct Nodes {
    nodes: Vec<(i32, i32, i32)>,
    truncated: bool,
}

fn database_error(err: sea_orm::DbErr) -> StatusCode {
    tracing::warn!(%err, "Atlas resource database lookup failed");
    StatusCode::INTERNAL_SERVER_ERROR
}

async fn nodes(
    State(state): State<AppState>,
    Query(params): Query<Params>,
) -> Result<impl IntoResponse, StatusCode> {
    let (ids, regions) = params.filters().ok_or(StatusCode::BAD_REQUEST)?;
    let (center_x, center_z) = params.center().ok_or(StatusCode::BAD_REQUEST)?;
    let ready = SyncInfo::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT count(*) = cardinality($1::smallint[]) AS ready
         FROM atlas_resource_region WHERE region = ANY($1::smallint[])",
        [regions.clone().into()],
    ))
    .one(&state.conn)
    .await
    .map_err(database_error)?;
    if !ready.is_some_and(|info| info.ready) {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    // Rank by distance in the staggered hex grid: twice-X units have
    // one third the squared world-space length of a Z row.
    let rows = Node::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT x, z, resource_id FROM (
             SELECT x, z, resource_id FROM atlas_resource_entry
             WHERE region = ANY($1::smallint[]) AND resource_id = ANY($2::integer[])
               AND x BETWEEN $3 AND $4 AND z BETWEEN $5 AND $6
             UNION
             SELECT n.x, n.z, n.resource_id FROM atlas_resource_node n
             JOIN atlas_resource_region r ON r.region = n.region AND NOT r.subscription_synced
             WHERE n.region = ANY($1::smallint[]) AND n.resource_id = ANY($2::integer[])
               AND n.x BETWEEN $3 AND $4 AND n.z BETWEEN $5 AND $6
         ) nodes
         ORDER BY power((2*x + (z & 1) - 2*$8::integer)::double precision, 2)
                + 3*power((z - $9::integer)::double precision, 2),
                  x, z, resource_id
         LIMIT $7",
        [
            regions.into(),
            ids.into(),
            params.min_x.into(),
            params.max_x.into(),
            params.min_z.into(),
            params.max_z.into(),
            ((LIMIT + 1) as i64).into(),
            center_x.into(),
            center_z.into(),
        ],
    ))
    .all(&state.conn)
    .await
    .map_err(database_error)?;
    let truncated = rows.len() > LIMIT;
    let nodes = rows
        .into_iter()
        .take(LIMIT)
        .map(|row| (row.x, row.z, row.resource_id))
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=15")],
        Json(Nodes { nodes, truncated }),
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClusterParams {
    min_x: i32,
    min_z: i32,
    max_x: i32,
    max_z: i32,
    level: i32,
    ids: String,
    regions: String,
}

impl ClusterParams {
    fn filters(&self) -> Option<(Vec<i32>, Vec<Region>)> {
        if !(0..=MAX_LEVEL).contains(&self.level) {
            return None;
        }
        let max = 96_000 / (BASE << self.level);
        if !(0..=max).contains(&self.min_x)
            || !(self.min_x..=max).contains(&self.max_x)
            || !(0..=max).contains(&self.min_z)
            || !(self.min_z..=max).contains(&self.max_z)
            || (self.max_x - self.min_x + 1) * (self.max_z - self.min_z + 1) > 4096
        {
            return None;
        }
        let regions: Vec<Region> = parse_list(&self.regions, 25)?;
        if regions.iter().any(|&region| !(1..=255).contains(&region)) {
            return None;
        }
        Some((parse_list(&self.ids, MAX_TYPES)?, regions))
    }
}

#[derive(FromQueryResult)]
struct ClusterRow {
    bucket_x: i32,
    bucket_z: i32,
    node_count: i64,
    resource_id: i32,
    center_x2: f64,
    center_z: f64,
}

#[derive(Serialize)]
struct Clusters {
    clusters: Vec<(i32, i32, i64, i32, f64, f64)>,
    partial: bool,
}

#[derive(FromQueryResult)]
struct ClusterReady {
    ready: i64,
}

async fn clusters(
    State(state): State<AppState>,
    Query(params): Query<ClusterParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let (ids, regions) = params.filters().ok_or(StatusCode::BAD_REQUEST)?;
    let ready = ClusterReady::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT count(*) AS ready FROM atlas_resource_region
         WHERE region = ANY($1::smallint[]) AND clusters_synced",
        [regions.clone().into()],
    ))
    .one(&state.conn)
    .await
    .map_err(database_error)?
    .map_or(0, |info| info.ready);
    if ready == 0 {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    let partial = ready < regions.len() as i64;
    let rows = ClusterRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "WITH types AS (
             SELECT bucket_x, bucket_z, resource_id, sum(node_count) AS n,
                    sum(sum_x2) AS sx2, sum(sum_z) AS sz
             FROM atlas_resource_cluster JOIN atlas_resource_region USING (region)
             WHERE region = ANY($1::smallint[]) AND clusters_synced
               AND level = $2 AND resource_id = ANY($3::integer[])
               AND bucket_x BETWEEN $4 AND $5 AND bucket_z BETWEEN $6 AND $7
             GROUP BY bucket_x, bucket_z, resource_id
         )
         SELECT bucket_x, bucket_z, sum(n)::bigint AS node_count,
                 (array_agg(resource_id ORDER BY n DESC, resource_id))[1] AS resource_id,
                 (sum(sx2) / sum(n))::double precision AS center_x2,
                 (sum(sz) / sum(n))::double precision AS center_z
         FROM types GROUP BY bucket_x, bucket_z HAVING sum(n) > 0",
        [
            regions.into(),
            params.level.into(),
            ids.into(),
            params.min_x.into(),
            params.max_x.into(),
            params.min_z.into(),
            params.max_z.into(),
        ],
    ))
    .all(&state.conn)
    .await
    .map_err(database_error)?;
    let clusters = rows
        .into_iter()
        .map(|row| {
            (
                row.bucket_x,
                row.bucket_z,
                row.node_count,
                row.resource_id,
                row.center_x2,
                row.center_z,
            )
        })
        .collect();
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=15")],
        Json(Clusters { clusters, partial }),
    ))
}

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/world-map/resource-types", get(catalog))
        .route("/world-map/resources", get(nodes))
        .route("/world-map/resource-clusters", get(clusters))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use sea_orm::{MockDatabase, Value};
    use std::collections::BTreeMap;
    use tower::ServiceExt;

    #[tokio::test]
    async fn only_reads_postgres_and_waits_for_initial_sync() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                vec![BTreeMap::from([("ready", Value::from(false))])],
                vec![BTreeMap::from([("ready", Value::from(true))])],
                vec![BTreeMap::from([
                    ("x", Value::from(100_i32)),
                    ("z", Value::from(300_i32)),
                    ("resource_id", Value::from(12_i32)),
                ])],
            ])
            .into_connection();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = routes().with_state(AppState::new(db, tx, Default::default()));
        for (ids, status) in [
            ("bogus", StatusCode::BAD_REQUEST),
            ("12&centerX=200&centerZ=310", StatusCode::BAD_REQUEST),
            ("12&centerX=120", StatusCode::BAD_REQUEST),
            ("12", StatusCode::SERVICE_UNAVAILABLE),
            ("12,-1,12", StatusCode::OK),
        ] {
            let response = app.clone().oneshot(Request::builder()
                .uri(format!("/world-map/resources?minX=96&maxX=180&minZ=300&maxZ=370&regions=7&ids={ids}"))
                .body(Body::empty()).unwrap()).await.unwrap();
            assert_eq!(response.status(), status);
            if status == StatusCode::OK {
                let content = to_bytes(response.into_body(), 4096).await.unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&content).unwrap(),
                    serde_json::json!({"nodes":[[100,300,12]],"truncated":false})
                );
            }
        }
    }

    #[tokio::test]
    async fn clusters_allow_world_views_and_report_partial_coverage() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                vec![BTreeMap::from([("ready", Value::from(1_i64))])],
                vec![BTreeMap::from([
                    ("bucket_x", Value::from(3_i32)),
                    ("bucket_z", Value::from(7_i32)),
                    ("node_count", Value::from(64_000_i64)),
                    ("resource_id", Value::from(12_i32)),
                    ("center_x2", Value::from(19000.5_f64)),
                    ("center_z", Value::from(22_000_f64)),
                ])],
            ])
            .into_connection();
        let (tx, _) = tokio::sync::mpsc::unbounded_channel();
        let app = routes().with_state(AppState::new(db, tx, Default::default()));
        for (level, max, status) in [(0, 2000, StatusCode::BAD_REQUEST), (6, 31, StatusCode::OK)] {
            let response = app.clone().oneshot(Request::builder()
                .uri(format!("/world-map/resource-clusters?minX=0&minZ=0&maxX={max}&maxZ={max}&level={level}&regions=7,8,9,12,13,14,17,18,19&ids=12"))
                .body(Body::empty()).unwrap()).await.unwrap();
            assert_eq!(response.status(), status);
            if status == StatusCode::OK {
                let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                    serde_json::json!({"clusters":[[3,7,64000,12,19000.5,22000.0]],"partial":true})
                );
            }
        }
    }
}
