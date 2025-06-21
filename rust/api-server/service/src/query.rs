use ::entity::building_state;
use ::entity::cargo_desc;
use ::entity::claim_member_state;
use ::entity::claim_tech_desc;
use ::entity::claim_tech_state;
use ::entity::collectible_desc;
use ::entity::collectible_desc::CollectibleType;
use ::entity::crafting_recipe;
use ::entity::deployable_state;
use ::entity::inventory;
use ::entity::inventory_changelog;
use ::entity::inventory_changelog::ItemType;
use ::entity::trade_order;
use ::entity::traveler_task_state;
use ::entity::vault_state_collectibles;
use ::entity::vault_state_collectibles::VaultStateCollectibleWithDesc;
use ::entity::{
    building_desc, claim_state, experience_state, item_desc, item_desc::Entity as Item,
    player_state, player_state::Entity as PlayerState, player_username_state,
    player_username_state::Entity as PlayerUsernameState, skill_desc,
};
use sea_orm::sea_query::extension::postgres::PgExpr;
use sea_orm::sea_query::{Alias, Expr, ExprTrait, PgFunc, PostgresQueryBuilder};
use sea_orm::*;

pub struct Query;

impl Query {
    pub async fn find_player_by_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<player_state::Model>, DbErr> {
        PlayerState::find_by_id(id).one(db).await
    }

    pub async fn find_player_by_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<player_state::Model>, DbErr> {
        PlayerState::find()
            .filter(player_state::Column::EntityId.is_in(ids))
            .all(db)
            .await
    }

    pub async fn find_player_username_by_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<player_username_state::Model>, DbErr> {
        PlayerUsernameState::find()
            .filter(player_username_state::Column::EntityId.is_in(ids))
            .all(db)
            .await
    }

    /// If ok, returns (post models, num pages).
    pub async fn find_players(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
        online: Option<bool>,
    ) -> Result<
        (
            Vec<player_state::Model>,
            Vec<player_username_state::Model>,
            ItemsAndPagesNumber,
        ),
        DbErr,
    > {
        // Setup paginator
        let paginator = PlayerState::find()
            .join_rev(
                JoinType::LeftJoin,
                player_username_state::Entity::belongs_to(player_state::Entity)
                    .from(player_username_state::Column::EntityId)
                    .to(player_state::Column::EntityId)
                    .into(),
            )
            .order_by_asc(player_username_state::Column::Username)
            .apply_if(search, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => query.filter(
                    Expr::col(player_username_state::Column::Username)
                        .ilike(format!("%{}%", value)),
                ),
                _ => unreachable!(),
            })
            .apply_if(online, |query, value| {
                query.filter(
                    player_username_state::Column::EntityId.in_subquery(
                        PlayerState::find()
                            .select_only()
                            .column(player_state::Column::EntityId)
                            .filter(player_state::Column::SignedIn.eq(value))
                            .into_query(),
                    ),
                )
            })
            .paginate(db, per_page);

        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        let (player_states, num_pages) = paginator
            .fetch_page(page - 1)
            .await
            .map(|p| (p, num_pages))?;

        let player_usernames = PlayerUsernameState::find()
            .filter(
                player_username_state::Column::EntityId.is_in(
                    player_states
                        .iter()
                        .map(|p| p.entity_id)
                        .collect::<Vec<i64>>(),
                ),
            )
            .all(db)
            .await?;

        Ok((player_states, player_usernames, num_pages))
    }

    // /// If ok, returns (post models, num pages).
    // pub async fn find_locations(
    //     db: &DbConn,
    //     page: u64,
    //     posts_per_page: u64,
    // ) -> Result<(Vec<location::Model>, ItemsAndPagesNumber), DbErr> {
    //     // Setup paginator
    //     let paginator = Location::find()
    //         .order_by_asc(location::Column::EntityId)
    //         .paginate(db, posts_per_page);
    //     let num_pages = paginator.num_items_and_pages().await?;
    //
    //     // Fetch paginated posts
    //     paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    // }

    /// If ok, returns (post models, num pages).
    pub async fn find_items(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
    ) -> Result<(Vec<item_desc::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let paginator = Item::find()
            .order_by_asc(item_desc::Column::Id)
            .apply_if(search, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => {
                    query.filter(Expr::col(item_desc::Column::Name).ilike(format!("%{}%", value)))
                }
                _ => unreachable!(),
            })
            .paginate(db, per_page);

        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn all_items(db: &DbConn) -> Result<Vec<item_desc::Model>, DbErr> {
        Item::find().all(db).await
    }

    pub async fn search_items_desc(
        db: &DbConn,
        search: &Option<String>,
        tier: &Option<i32>,
        tag: &Option<String>,
        no_item_list: &Option<bool>,
    ) -> Result<Vec<item_desc::Model>, DbErr> {
        Item::find()
            .apply_if(search.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => query
                        .filter(Expr::col(item_desc::Column::Name).ilike(format!("%{}%", value))),
                    _ => unreachable!(),
                }
            })
            .apply_if(tag.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => {
                        query.filter(Expr::col(item_desc::Column::Tag).eq(value))
                    }
                    _ => unreachable!(),
                }
            })
            .apply_if(*tier, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => query.filter(Expr::col(item_desc::Column::Tier).eq(value)),
                _ => unreachable!(),
            })
            .apply_if(*no_item_list, |query, _| match db.get_database_backend() {
                DbBackend::Postgres => {
                    query.filter(Expr::col(item_desc::Column::ItemListId).is_not(0))
                }
                _ => unreachable!(),
            })
            .all(db)
            .await
    }

    pub async fn find_item_by_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<item_desc::Model>, DbErr> {
        Item::find()
            .filter(item_desc::Column::Id.is_in(ids))
            .all(db)
            .await
    }

    pub async fn search_items_desc_ids(
        db: &DbConn,
        search: &Option<String>,
    ) -> Result<Vec<i64>, DbErr> {
        Item::find()
            .select_only()
            .column(item_desc::Column::Id)
            .apply_if(search.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => query
                        .filter(Expr::col(item_desc::Column::Name).ilike(format!("%{}%", value))),
                    _ => unreachable!(),
                }
            })
            .order_by_asc(item_desc::Column::Id)
            .into_tuple()
            .all(db)
            .await
    }

    pub async fn find_unique_item_tags(db: &DbConn) -> Result<Vec<String>, DbErr> {
        let items = Item::find()
            .select_only()
            .column(item_desc::Column::Tag)
            .group_by(item_desc::Column::Tag)
            .order_by_asc(item_desc::Column::Tag)
            .into_model::<ItemTag>()
            .all(db)
            .await?;
        Ok(items.into_iter().map(|item| item.tag).collect())
    }

    pub async fn find_unique_item_tiers(db: &DbConn) -> Result<Vec<i32>, DbErr> {
        let items = Item::find()
            .select_only()
            .column(item_desc::Column::Tier)
            .group_by(item_desc::Column::Tier)
            .order_by_asc(item_desc::Column::Tier)
            .into_model::<ItemTier>()
            .all(db)
            .await?;
        Ok(items.into_iter().map(|item| item.tier).collect())
    }

    pub async fn all_cargos_desc(db: &DbConn) -> Result<Vec<cargo_desc::Model>, DbErr> {
        cargo_desc::Entity::find().all(db).await
    }

    pub async fn search_cargos_desc(
        db: &DbConn,
        search: &Option<String>,
        tier: &Option<i32>,
        tag: &Option<String>,
    ) -> Result<Vec<cargo_desc::Model>, DbErr> {
        cargo_desc::Entity::find()
            .apply_if(search.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => query
                        .filter(Expr::col(cargo_desc::Column::Name).ilike(format!("%{}%", value))),
                    _ => unreachable!(),
                }
            })
            .apply_if(tag.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => {
                        query.filter(Expr::col(cargo_desc::Column::Tag).eq(value))
                    }
                    _ => unreachable!(),
                }
            })
            .apply_if(*tier, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => query.filter(Expr::col(cargo_desc::Column::Tier).eq(value)),
                _ => unreachable!(),
            })
            .all(db)
            .await
    }

    pub async fn search_cargo_desc_ids(
        db: &DbConn,
        search: &Option<String>,
    ) -> Result<Vec<i64>, DbErr> {
        cargo_desc::Entity::find()
            .select_only()
            .column(cargo_desc::Column::Id)
            .apply_if(search.clone(), |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => query
                        .filter(Expr::col(cargo_desc::Column::Name).ilike(format!("%{}%", value))),
                    _ => unreachable!(),
                }
            })
            .order_by_asc(cargo_desc::Column::Id)
            .into_tuple()
            .all(db)
            .await
    }

    pub async fn find_cargo_by_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<cargo_desc::Model>, DbErr> {
        cargo_desc::Entity::find()
            .filter(cargo_desc::Column::Id.is_in(ids))
            .all(db)
            .await
    }

    pub async fn find_unique_cargo_tags(db: &DbConn) -> Result<Vec<String>, DbErr> {
        let items = cargo_desc::Entity::find()
            .select_only()
            .column(cargo_desc::Column::Tag)
            .group_by(cargo_desc::Column::Tag)
            .order_by_asc(cargo_desc::Column::Tag)
            .into_model::<CargoTag>()
            .all(db)
            .await?;
        Ok(items.into_iter().map(|item| item.tag).collect())
    }

    pub async fn find_unique_cargo_tiers(db: &DbConn) -> Result<Vec<i32>, DbErr> {
        let items = cargo_desc::Entity::find()
            .select_only()
            .column(cargo_desc::Column::Tier)
            .group_by(cargo_desc::Column::Tier)
            .order_by_asc(cargo_desc::Column::Tier)
            .into_model::<CargoTier>()
            .all(db)
            .await?;
        Ok(items.into_iter().map(|item| item.tier).collect())
    }

    /// If ok, returns (post models, num pages).
    pub async fn find_claim_descriptions(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
        has_research: Option<i32>,
        is_running_upgrade: Option<bool>,
    ) -> Result<(Vec<claim_state::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let paginator = claim_state::Entity::find()
            .order_by_desc(Expr::cust("boost"))
            .order_by_asc(claim_state::Column::EntityId)
            .expr_as(
                Expr::case(
                    Expr::eq(
                        Expr::val(200),
                        Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                    ),
                    1000,
                )
                .finally(0)
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(300),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(400),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(500),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(600),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(700),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(800),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(900),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1000),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        1000,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2136582402),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1426269264),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1912870048),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(187931146),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(736567281),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1728735124),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(905455362),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(571236380),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(830936030),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1826500486),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1157053499),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(688169271),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(733358069),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(906602291),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1273121576),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1206083197),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1567647942),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(649819263),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1161565644),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(477458683),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2099304788),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(956489539),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(473237479),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(728778297),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1617681454),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1337780142),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2072167322),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(10000),
                            Expr::expr(PgFunc::any(Expr::col(claim_tech_state::Column::Learned))),
                        ),
                        5,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(200),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(300),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(400),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(500),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(600),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(700),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(800),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(900),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1000),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        100,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2136582402),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1426269264),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1912870048),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(187931146),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(736567281),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1728735124),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(905455362),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(571236380),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(830936030),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1826500486),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1157053499),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(688169271),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(733358069),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(906602291),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1273121576),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1206083197),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1567647942),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(649819263),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1161565644),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(477458683),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2099304788),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(956489539),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(473237479),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(728778297),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1617681454),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(1337780142),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(2072167322),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .add(
                    Expr::case(
                        Expr::eq(
                            Expr::val(10000),
                            Expr::expr(Expr::col(claim_tech_state::Column::Researching)),
                        ),
                        1,
                    )
                    .finally(0),
                )
                .into_simple_expr(),
                "boost",
            )
            .join_rev(
                JoinType::LeftJoin,
                claim_tech_state::Entity::belongs_to(claim_state::Entity)
                    .from(claim_tech_state::Column::EntityId)
                    .to(claim_state::Column::EntityId)
                    .into(),
            )
            .filter(claim_state::Column::Neutral.ne(true))
            // .filter(claim_description_state::Column::OwnerPlayerEntityId.ne(0))
            .apply_if(search, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => {
                    query.filter(Expr::col(claim_state::Column::Name).ilike(format!("%{}%", value)))
                }
                _ => unreachable!(),
            })
            // Look at how to write this query so it works and seo-orm does not to make things I would not like it do to here.
            // The query needs to look like this at the end: SELECT "entity_id" FROM "claim_tech_state" WHERE learned::jsonb @> '[500]';
            .apply_if(has_research, |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => {
                        query.filter(
                            Condition::any().add(
                                claim_state::Column::EntityId.in_subquery(
                                    sea_query::Query::select()
                                        .column(claim_tech_state::Column::EntityId)
                                        // .and_where(SimpleExpr::from(PgFunc::any(Expr::col(claim_tech_state::Column::Learned)).eq(
                                        //     value
                                        // )))
                                        .and_where(Expr::eq(
                                            Expr::val(value),
                                            Expr::expr(PgFunc::any(Expr::col(
                                                claim_tech_state::Column::Learned,
                                            ))),
                                        ))
                                        .from(claim_tech_state::Entity)
                                        .to_owned(),
                                ),
                            ),
                        )
                    }
                    _ => unreachable!(),
                }
            })
            .apply_if(is_running_upgrade, |query, value| {
                match db.get_database_backend() {
                    DbBackend::Postgres => {
                        let where_query = if value {
                            claim_tech_state::Column::Researching.ne(0)
                        } else {
                            claim_tech_state::Column::Researching.eq(0)
                        };

                        query.filter(
                            Condition::any().add(
                                claim_state::Column::EntityId.in_subquery(
                                    sea_query::Query::select()
                                        .column(claim_tech_state::Column::EntityId)
                                        .and_where(where_query)
                                        .from(claim_tech_state::Entity)
                                        .to_owned(),
                                ),
                            ),
                        )
                    }
                    _ => unreachable!(),
                }
            })
            .paginate(db, per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn find_claim_state(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<claim_state::Model>, DbErr> {
        claim_state::Entity::find_by_id(id).one(db).await
    }

    pub async fn find_claim_member_by_claim_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<claim_member_state::Model>, DbErr> {
        claim_member_state::Entity::find()
            .filter(claim_member_state::Column::ClaimEntityId.eq(id))
            .all(db)
            .await
    }

    pub async fn skill_descriptions(db: &DbConn) -> Result<Vec<skill_desc::Model>, DbErr> {
        skill_desc::Entity::find().all(db).await
    }

    pub async fn full_experience_states_by_skill_id(
        db: &DbConn,
        skill_id: i64,
    ) -> Result<Vec<experience_state::Model>, DbErr> {
        experience_state::Entity::find()
            .order_by_asc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .all(db)
            .await
    }

    pub async fn get_experience_state_top_100_by_skill_id(
        db: &DbConn,
        skill_id: i64,
        exclude: Option<[i64; 1]>,
    ) -> Result<Vec<experience_state::Model>, DbErr> {
        experience_state::Entity::find()
            .order_by_desc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .filter(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .limit(100)
            .all(db)
            .await
    }

    pub async fn get_experience_state_player_ids_by_skill_id(
        db: &DbConn,
        skill_id: i64,
        player_ids: Vec<i64>,
        exclude: Option<[i64; 1]>,
    ) -> Result<Vec<experience_state::Model>, DbErr> {
        experience_state::Entity::find()
            .order_by_desc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .filter(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .filter(experience_state::Column::EntityId.is_in(player_ids))
            .all(db)
            .await
    }

    pub async fn get_experience_state_player_by_skill_id(
        db: &DbConn,
        skill_id: i64,
        player_id: i64,
        exclude: Option<[i64; 1]>,
    ) -> Result<(Option<experience_state::Model>, Option<u64>), DbErr> {
        let player_experience = experience_state::Entity::find()
            .order_by_desc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .filter(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .filter(experience_state::Column::EntityId.eq(player_id))
            .one(db)
            .await?;

        if player_experience.is_none() {
            return Ok((None, None));
        }

        let player_experience = player_experience.unwrap();

        let rank = experience_state::Entity::find()
            .order_by_desc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .filter(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .filter(experience_state::Column::Experience.gte(player_experience.experience))
            .count(db)
            .await?;

        Ok((Some(player_experience), Some(rank)))
    }

    pub async fn get_experience_state_top_100_total_level(
        db: &DbConn,
        level_case_sql: String,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<(u64, i32)>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust(level_case_sql), Alias::new("level"))
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("level"), Order::Desc)
            .limit(100)
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let level: i64 = row.try_get("", "level").unwrap();
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                (entity_id.try_into().unwrap(), level.try_into().unwrap())
            })
            .collect::<Vec<(u64, i32)>>())
    }

    pub async fn get_experience_state_player_ids_total_level(
        db: &DbConn,
        level_case_sql: String,
        player_ids: Vec<i64>,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<(u64, i32)>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust(level_case_sql), Alias::new("level"))
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("level"), Order::Desc)
            .and_where(experience_state::Column::EntityId.is_in(player_ids))
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let level: i64 = row.try_get("", "level").unwrap();
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                (entity_id as u64, level.try_into().unwrap())
            })
            .collect::<Vec<(u64, i32)>>())
    }

    pub async fn get_experience_state_player_level(
        db: &DbConn,
        level_case_sql: String,
        player_id: i64,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<(Option<u64>, Option<u64>), DbErr> {
        let query_level = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust(&level_case_sql), Alias::new("level"))
            .from(experience_state::Entity)
            .and_where(Expr::col(experience_state::Column::EntityId).eq(player_id))
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("level"), Order::Desc)
            .to_owned();

        let query_level = match db.get_database_backend() {
            DbBackend::Postgres => query_level.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        let level = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                query_level,
            ))
            .await?
            .into_iter()
            .map(|row| {
                let level: i64 = row.try_get("", "level").unwrap();
                u64::try_from(level).unwrap()
            })
            .collect::<Vec<u64>>();
        let level = *level.first().unwrap();

        let query_rank = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust(level_case_sql), Alias::new("level"))
            .from(experience_state::Entity)
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .group_by_col(experience_state::Column::EntityId)
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .order_by_expr(Expr::cust("level"), Order::Desc)
            .to_owned();

        let query_rank = sea_orm::sea_query::Query::select()
            .expr_as(Expr::cust("count(*)"), Alias::new("count"))
            .from_subquery(query_rank, Alias::new("subquery"))
            .and_where(Expr::col(Alias::new("level")).gte(level))
            .to_owned();

        let query_rank = match db.get_database_backend() {
            DbBackend::Postgres => query_rank.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        let rank: Option<i64> = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                query_rank,
            ))
            .await?
            .unwrap()
            .try_get("", "count")?;

        let rank = rank.unwrap() as u64;

        Ok((Some(level), Some(rank)))
    }

    pub async fn get_experience_state_player_rank_total_experience(
        db: &DbConn,
        player_id: i64,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<(Option<u64>, Option<u64>), DbErr> {
        let query_experience = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(
                Expr::cust("sum(experience)"),
                Alias::new("total_experience"),
            )
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .and_where(Expr::col(experience_state::Column::EntityId).eq(player_id))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .to_owned();

        let query_experience = match db.get_database_backend() {
            DbBackend::Postgres => query_experience.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        let experience: Option<i64> = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                query_experience,
            ))
            .await?
            .unwrap()
            .try_get("", "total_experience")?;
        let experience = experience.unwrap();

        let query_rank = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(
                Expr::cust("sum(experience)"),
                Alias::new("total_experience"),
            )
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .and_where(experience_state::Column::EntityId.is_not_in(exclude.unwrap_or([0])))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .to_owned();

        let query_rank = sea_orm::sea_query::Query::select()
            .expr_as(Expr::cust("count(*)"), Alias::new("count"))
            .from_subquery(query_rank, Alias::new("subquery"))
            .and_where(Expr::col(Alias::new("total_experience")).gte(experience))
            .to_owned();

        let query_rank = match db.get_database_backend() {
            DbBackend::Postgres => query_rank.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        let rank: Option<i64> = db
            .query_one(Statement::from_string(
                db.get_database_backend(),
                query_rank,
            ))
            .await?
            .unwrap()
            .try_get("", "count")?;

        let rank = rank.unwrap() as u64;

        Ok((Some(u64::try_from(experience).unwrap()), Some(rank)))
    }

    pub async fn get_experience_state_top_100_total_experience(
        db: &DbConn,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<(i64, i64, i64)>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column((Alias::new("es"), experience_state::Column::EntityId))
            .expr_as(
                Expr::cust("sum(experience)"),
                Alias::new("total_experience"),
            )
            .expr_as(
                Expr::cust(
                    "case when any_value(ps.time_signed_in) <= 3600 then 0
            when sum(experience) = 0 then 0
            else sum(experience) / (any_value(ps.time_signed_in)/ 3600) END",
                ),
                Alias::new("experience_per_hour"),
            )
            .and_where(
                Expr::col((Alias::new("es"), experience_state::Column::EntityId))
                    .is_not_in(exclude.unwrap_or([0])),
            )
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    Expr::col((Alias::new("es"), experience_state::Column::SkillId))
                        .not_in_subquery(
                            skill_desc::Entity::find()
                                .select_only()
                                .filter(skill_desc::Column::SkillCategory.is_in(value))
                                .column(skill_desc::Column::Id)
                                .into_query(),
                        ),
                );
            })
            .from_as(experience_state::Entity, Alias::new("es"))
            .join_as(
                JoinType::InnerJoin,
                player_state::Entity,
                Alias::new("ps"),
                Expr::col((Alias::new("es"), experience_state::Column::EntityId))
                    .equals((Alias::new("ps"), player_state::Column::EntityId)),
            )
            .group_by_col((Alias::new("es"), experience_state::Column::EntityId))
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .limit(100)
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                let total_experience: i64 = row.try_get("", "total_experience").unwrap();
                let experience_per_hour: i64 = row.try_get("", "experience_per_hour").unwrap();

                (entity_id, total_experience, experience_per_hour)
            })
            .collect())
    }

    pub async fn get_experience_state_top_100_experience_per_hour(
        db: &DbConn,
        exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<experience_state::Model>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column((Alias::new("es"), experience_state::Column::EntityId))
            .expr_as(
                Expr::cust(
                    "case when any_value(ps.time_signed_in) <= 3600 then 0
            when sum(experience) = 0 then 0
            else sum(experience) / (any_value(ps.time_signed_in)/ 3600) END",
                ),
                Alias::new("total_experience"),
            )
            .and_where(
                Expr::col((Alias::new("es"), experience_state::Column::EntityId))
                    .is_not_in(exclude.unwrap_or([0])),
            )
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    Expr::col((Alias::new("es"), experience_state::Column::SkillId))
                        .not_in_subquery(
                            skill_desc::Entity::find()
                                .select_only()
                                .filter(skill_desc::Column::SkillCategory.is_in(value))
                                .column(skill_desc::Column::Id)
                                .into_query(),
                        ),
                );
            })
            .from_as(experience_state::Entity, Alias::new("es"))
            .join_as(
                JoinType::InnerJoin,
                player_state::Entity,
                Alias::new("ps"),
                Expr::col((Alias::new("es"), experience_state::Column::EntityId))
                    .equals((Alias::new("ps"), player_state::Column::EntityId)),
            )
            .group_by_col((Alias::new("es"), experience_state::Column::EntityId))
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .limit(100)
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                let total_experience: i64 = row.try_get("", "total_experience").unwrap();

                experience_state::Model {
                    entity_id,
                    experience: total_experience.try_into().unwrap(),
                    skill_id: 1,
                }
            })
            .collect())
    }

    pub async fn get_experience_state_player_ids_total_experience(
        db: &DbConn,
        player_ids: Vec<i64>,
        _exclude: Option<[i64; 1]>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<experience_state::Model>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(
                Expr::cust("sum(experience)"),
                Alias::new("total_experience"),
            )
            .from(experience_state::Entity)
            .and_where(experience_state::Column::EntityId.is_in(player_ids))
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    experience_state::Column::SkillId.not_in_subquery(
                        skill_desc::Entity::find()
                            .select_only()
                            .filter(skill_desc::Column::SkillCategory.is_in(value))
                            .column(skill_desc::Column::Id)
                            .into_query(),
                    ),
                );
            })
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                let total_experience: i64 = row.try_get("", "total_experience").unwrap();

                experience_state::Model {
                    entity_id,
                    experience: total_experience.try_into().unwrap(),
                    skill_id: 1,
                }
            })
            .collect())
    }

    pub async fn get_experience_state_player_ids_total_experience_with_experience_per_hour(
        db: &DbConn,
        player_ids: Vec<i64>,
        excluded_skill_category: Option<[i64; 2]>,
    ) -> Result<Vec<(i64, i64, i64)>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column((Alias::new("es"), experience_state::Column::EntityId))
            .expr_as(
                Expr::cust("sum(experience)"),
                Alias::new("total_experience"),
            )
            .expr_as(
                Expr::cust(
                    "case when any_value(ps.time_signed_in) <= 3600 then 0
            when sum(experience) = 0 then 0
            else sum(experience) / (any_value(ps.time_signed_in)/ 3600) END",
                ),
                Alias::new("experience_per_hour"),
            )
            .apply_if(excluded_skill_category, |query, value| {
                query.and_where(
                    Expr::col((Alias::new("es"), experience_state::Column::SkillId))
                        .not_in_subquery(
                            skill_desc::Entity::find()
                                .select_only()
                                .filter(skill_desc::Column::SkillCategory.is_in(value))
                                .column(skill_desc::Column::Id)
                                .into_query(),
                        ),
                );
            })
            .and_where(
                Expr::col((Alias::new("es"), experience_state::Column::EntityId)).is_in(player_ids),
            )
            .from_as(experience_state::Entity, Alias::new("es"))
            .join_as(
                JoinType::InnerJoin,
                player_state::Entity,
                Alias::new("ps"),
                Expr::col((Alias::new("es"), experience_state::Column::EntityId))
                    .equals((Alias::new("ps"), player_state::Column::EntityId)),
            )
            .group_by_col((Alias::new("es"), experience_state::Column::EntityId))
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .to_owned();

        let query = match db.get_database_backend() {
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            _ => unreachable!(),
        };

        Ok(db
            .query_all(Statement::from_string(db.get_database_backend(), query))
            .await?
            .into_iter()
            .map(|row| {
                let entity_id: i64 = row.try_get("", "entity_id").unwrap();
                let total_experience: i64 = row.try_get("", "total_experience").unwrap();
                let experience_per_hour: i64 = row.try_get("", "experience_per_hour").unwrap();

                (entity_id, total_experience, experience_per_hour)
            })
            .collect())
    }

    pub async fn load_all_recipes(db: &DbConn) -> Vec<crafting_recipe::Model> {
        let query = crafting_recipe::Entity::find();
        query.all(db).await.unwrap()
    }

    pub async fn find_building_desc_by_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<building_desc::Model>, DbErr> {
        building_desc::Entity::find_by_id(id).one(db).await
    }

    pub async fn find_building_descs(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
    ) -> Result<(Vec<building_desc::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let paginator = building_desc::Entity::find()
            .order_by_asc(building_desc::Column::Id)
            .apply_if(search, |query, value| match db.get_database_backend() {
                DbBackend::Postgres => query
                    .filter(Expr::col(building_desc::Column::Name).ilike(format!("%{}%", value))),
                _ => unreachable!(),
            })
            .paginate(db, per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn find_building_descs_with_inventory(
        db: &DbConn,
    ) -> Result<Vec<building_desc::Model>, DbErr> {
        Ok(building_desc::Entity::find()
            .all(db)
            .await?
            .into_iter()
            .filter(|building_desc| {
                building_desc
                    .functions
                    .iter()
                    .any(|function| function.cargo_slots > 0 || function.storage_slots > 0)
            })
            .collect())
    }

    pub async fn find_building_state_by_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<building_state::Model>, DbErr> {
        building_state::Entity::find_by_id(id).one(db).await
    }

    pub async fn find_building_state_by_claim_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<building_state::Model>, DbErr> {
        building_state::Entity::find()
            .filter(building_state::Column::ClaimEntityId.eq(id))
            .all(db)
            .await
    }

    pub async fn find_building_state_by_desc_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<building_state::Model>, DbErr> {
        building_state::Entity::find()
            .filter(building_state::Column::BuildingDescriptionId.is_in(ids))
            .all(db)
            .await
    }

    pub async fn find_building_states(
        db: &DbConn,
        page: u64,
        per_page: u64,
        id: Option<i64>,
        buildings_with_inventory_ids: Option<Vec<i64>>,
    ) -> Result<(Vec<building_state::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let paginator = building_state::Entity::find()
            .order_by_asc(building_state::Column::EntityId)
            .apply_if(id, |query, value| {
                query.filter(building_state::Column::ClaimEntityId.eq(value))
            })
            .apply_if(buildings_with_inventory_ids, |query, value| {
                query.filter(building_state::Column::BuildingDescriptionId.is_in(value))
            })
            .paginate(db, per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn find_inventory_by_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<inventory::Model>, DbErr> {
        inventory::Entity::find_by_id(id).one(db).await
    }

    pub async fn find_claim_tech_state_by_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<claim_tech_state::Model>, DbErr> {
        claim_tech_state::Entity::find_by_id(id).one(db).await
    }

    pub async fn find_claim_tech_state_by_ids(
        db: &DbConn,
        id: Vec<i64>,
    ) -> Result<Vec<claim_tech_state::Model>, DbErr> {
        claim_tech_state::Entity::find()
            .filter(claim_tech_state::Column::EntityId.is_in(id))
            .all(db)
            .await
    }

    pub async fn all_claim_tech_desc(db: &DbConn) -> Result<Vec<claim_tech_desc::Model>, DbErr> {
        claim_tech_desc::Entity::find().all(db).await
    }

    pub async fn find_inventory_by_owner_entity_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<(Vec<inventory::Model>, ItemsAndPagesNumber), DbErr> {
        let paginator = inventory::Entity::find()
            .filter(inventory::Column::OwnerEntityId.is_in(ids))
            .order_by_asc(inventory::Column::EntityId)
            .paginate(db, 24);

        let num_pages = paginator.num_items_and_pages().await?;

        paginator.fetch_page(0).await.map(|p| (p, num_pages))
    }

    pub async fn find_inventory_entity_ids_by_owner_entity_ids(
        db: &DbConn,
        ids: Vec<i64>,
    ) -> Result<Vec<i64>, DbErr> {
        inventory::Entity::find()
            .select_only()
            .column(inventory::Column::EntityId)
            .filter(inventory::Column::OwnerEntityId.is_in(ids))
            .order_by_asc(inventory::Column::EntityId)
            .into_tuple()
            .all(db)
            .await
    }

    pub async fn find_inventory_by_player_owner_entity_id(
        db: &DbConn,
        id: i64,
    ) -> Result<(Vec<inventory::Model>, ItemsAndPagesNumber), DbErr> {
        let paginator = inventory::Entity::find()
            .filter(inventory::Column::PlayerOwnerEntityId.eq(id))
            .order_by_asc(inventory::Column::EntityId)
            .paginate(db, 24);

        let num_pages = paginator.num_items_and_pages().await?;

        paginator.fetch_page(0).await.map(|p| (p, num_pages))
    }

    pub async fn get_inventorys_by_owner_entity_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<inventory::Model>, DbErr> {
        inventory::Entity::find()
            .filter(inventory::Column::OwnerEntityId.eq(id))
            .all(db)
            .await
    }

    pub async fn find_inventory_changes_by_entity_ids(
        db: &DbConn,
        ids: Vec<i64>,
        page_size: u64,
        item_id: Option<i32>,
        item_type: Option<ItemType>,
        user_id: Option<i64>,
    ) -> Result<(Vec<inventory_changelog::Model>, ItemsAndPagesNumber), DbErr> {
        let paginator = inventory_changelog::Entity::find()
            .filter(inventory_changelog::Column::EntityId.is_in(ids))
            .apply_if(item_id, |query, value| {
                query.filter(
                    Condition::any()
                        .add(inventory_changelog::Column::NewItemId.eq(value))
                        .add(inventory_changelog::Column::OldItemId.eq(value)),
                )
            })
            .apply_if(item_type, |query, value| {
                query.filter(
                    Condition::any()
                        .add(inventory_changelog::Column::NewItemType.eq(value.clone()))
                        .add(inventory_changelog::Column::OldItemType.eq(value)),
                )
            })
            .apply_if(user_id, |query, value| {
                query.filter(inventory_changelog::Column::UserId.eq(value.clone()))
            })
            .order_by_desc(inventory_changelog::Column::Timestamp)
            .paginate(db, page_size);

        let num_pages = paginator.num_items_and_pages().await?;

        paginator.fetch_page(0).await.map(|p| (p, num_pages))
    }

    pub async fn get_inventorys_by_player_owner_entity_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<inventory::Model>, DbErr> {
        inventory::Entity::find()
            .filter(inventory::Column::PlayerOwnerEntityId.eq(id))
            .all(db)
            .await
    }

    pub async fn get_inventorys_by_owner_entity_ids(
        db: &DbConn,
        id: Vec<i64>,
    ) -> Result<Vec<inventory::Model>, DbErr> {
        inventory::Entity::find()
            .filter(inventory::Column::OwnerEntityId.is_in(id))
            .all(db)
            .await
    }

    pub async fn find_deployable_entity_by_owner_entity_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<deployable_state::Model>, DbErr> {
        deployable_state::Entity::find()
            .filter(deployable_state::Column::OwnerId.eq(id))
            .all(db)
            .await
    }

    pub async fn get_traveler_task_state_by_player_entity_ids(
        db: &DbConn,
        id: Vec<i64>,
    ) -> Result<Vec<traveler_task_state::Model>, DbErr> {
        traveler_task_state::Entity::find()
            .filter(traveler_task_state::Column::PlayerEntityId.is_in(id))
            .all(db)
            .await
    }

    pub async fn find_vault_deployable_by_player_with_desc(
        db: &DbConn,
        id: i64,
    ) -> Result<Vec<VaultStateCollectibleWithDesc>, DbErr> {
        let collectible_descs = collectible_desc::Entity::find()
            .filter(collectible_desc::Column::CollectibleType.eq(CollectibleType::Deployable))
            .filter(
                collectible_desc::Column::Id.in_subquery(
                    vault_state_collectibles::Entity::find()
                        .select_only()
                        .column(vault_state_collectibles::Column::Id)
                        .filter(vault_state_collectibles::Column::EntityId.eq(id))
                        .into_query(),
                ),
            )
            .order_by_asc(collectible_desc::Column::Id)
            .all(db)
            .await?;

        let vault_state = vault_state_collectibles::Entity::find()
            .filter(vault_state_collectibles::Column::EntityId.eq(id))
            .filter(
                vault_state_collectibles::Column::Id.is_in(
                    collectible_descs
                        .iter()
                        .map(|collectible| collectible.id)
                        .collect::<Vec<i32>>(),
                ),
            )
            .order_by_asc(vault_state_collectibles::Column::Id)
            .all(db)
            .await?;

        Ok(collectible_descs
            .into_iter()
            .zip(vault_state)
            .map(|(desc, state)| VaultStateCollectibleWithDesc {
                entity_id: state.entity_id,
                id: state.id,
                activated: state.activated,
                count: state.count,
                collectible_desc: desc,
            })
            .collect())
    }

    pub async fn find_trade_order_by_items_or_cargo_ids(
        db: &DbConn,
        items: Vec<i32>,
        cargo_ids: Vec<i32>,
    ) -> Result<Vec<trade_order::Model>, DbErr> {
        trade_order::Entity::find()
            .filter(trade_order::Column::OfferItems.is_in(items))
            .filter(trade_order::Column::OfferCargoId.is_in(cargo_ids))
            .all(db)
            .await
    }

    pub async fn load_trade_order(db: &DbConn) -> Result<Vec<trade_order::Model>, DbErr> {
        trade_order::Entity::find()
            .order_by_asc(trade_order::Column::EntityId)
            .all(db)
            .await
    }

    pub async fn load_trade_order_paginated(
        db: &DbConn,
        page: u64,
        per_page: u64,
    ) -> Result<(Vec<trade_order::Model>, ItemsAndPagesNumber), DbErr> {
        let paginator = trade_order::Entity::find()
            .order_by_asc(trade_order::Column::EntityId)
            .paginate(db, per_page);

        let num_pages = paginator.num_items_and_pages().await?;

        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }
}

#[derive(FromQueryResult)]
struct ItemTag {
    pub tag: String,
}

#[derive(FromQueryResult)]
struct ItemTier {
    pub tier: i32,
}

#[derive(FromQueryResult)]
struct CargoTag {
    pub tag: String,
}

#[derive(FromQueryResult)]
struct CargoTier {
    pub tier: i32,
}
