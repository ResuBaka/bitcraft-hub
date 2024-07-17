use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_cookies::Cookies;
use entity::{claim_description};
use crate::{AppState, Params};
use service::{
    Mutation as MutationCore,
    Query as QueryCore, sea_orm::{Database, DatabaseConnection},
};
use std::fs::File;
use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, PaginatorTrait};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClaimDescription {
    pub entity_id: u64,
    pub owner_player_entity_id: u64,
    pub owner_building_entity_id: u64,
    pub name: String,
    pub supplies: f32,
    pub building_maintenance: f32,
    pub members: Vec<ClaimDescriptionMember>,
    pub tiles: i32,
    pub extensions: i32,
    pub neutral: bool,
    pub location: sea_orm::prelude::Json,
    pub treasury: i32,
}

impl From<claim_description::Model> for ClaimDescription {
    fn from(claim_description: claim_description::Model) -> Self {
        ClaimDescription {
            entity_id: claim_description.entity_id,
            owner_player_entity_id: claim_description.owner_player_entity_id,
            owner_building_entity_id: claim_description.owner_building_entity_id,
            name: claim_description.name,
            supplies: claim_description.supplies,
            building_maintenance: claim_description.building_maintenance,
            members: serde_json::from_value(claim_description.members).unwrap(),
            tiles: claim_description.tiles,
            extensions: claim_description.extensions,
            neutral: claim_description.neutral,
            location: claim_description.location,
            treasury: claim_description.treasury,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct ClaimDescriptionMember {
    pub entity_id: u64,
    pub user_name: String,
    pub inventory_permission: bool,
    pub build_permission: bool,
    pub officer_permission: bool,
    pub co_owner_permission: bool,
}


pub(crate) async fn list_claims(
    state: State<AppState>,
    Query(params): Query<Params>,
    cookies: Cookies,
) -> Result<Json<Value>, (StatusCode, &'static str)> {
    let page = params.page.unwrap_or(1);
    let posts_per_page = params.per_page.unwrap_or(5);
    let search = params.search;

    let (claims, num_pages) = QueryCore::find_claim_descriptions(&state.conn, page, posts_per_page, search)
        .await
        .expect("Cannot find posts in page");

    let claims = claims.into_iter().map(|claim_description| claim_description.into()).collect::<Vec<ClaimDescription>>();

    Ok(Json(json!({
        "claims": claims,
        "perPage": posts_per_page,
        "total": num_pages.number_of_items,
        "page": page,
    })))
}


pub(crate)async fn find_claim_descriptions(
    state: State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<ClaimDescription>, (StatusCode, &'static str)> {
    let posts = QueryCore::find_claim_description_by_id(&state.conn, id)
        .await
        .expect("Cannot find posts in page");

    if posts.is_none() {
        return Err((StatusCode::NOT_FOUND, "ClaimDescription not found"));
    }

    let posts: ClaimDescription = posts.unwrap().into();

    Ok(Json(posts))
}

pub(crate) async fn import_claim_description_state(
    conn: &DatabaseConnection,
) -> anyhow::Result<()> {
    let mut item_file = File::open("/home/resubaka/code/crafting-list/storage/State/ClaimDescriptionState.json").unwrap();
    let claim_descriptions: Value = serde_json::from_reader(&item_file).unwrap();
    let claim_descriptions: Vec<claim_description::Model> = serde_json::from_value(claim_descriptions.get(0).unwrap().get("rows").unwrap().clone()).unwrap();
    let count = claim_descriptions.len();
    let db_count = claim_description::Entity::find().count(conn).await.unwrap();

    if (count as u64) == db_count {
        println!("ClaimDescriptionState already imported");
        return Ok(());
    }

    let claim_descriptions: Vec<claim_description::ActiveModel> = claim_descriptions.into_iter().map(|x| x.into_active_model()).collect();

    for claim_description in claim_descriptions.chunks(5000) {
        let _ = claim_description::Entity::insert_many(claim_description.to_vec()).exec(conn).await;
    }

    Ok(())
}