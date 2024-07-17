use std::any::Any;
use ::entity::{
    player_state, player_state::Entity as PlayerState,
    item, item::Entity as Item,
    location, location::Entity as Location,
    claim_description, claim_description::Entity as ClaimDescription,
    skill_desc, skill_desc::Entity as SkillDesc,
    experience_state, experience_state::Entity as ExperienceState,
};
use sea_orm::*;
use sea_orm::prelude::Decimal;
use sea_orm::sea_query::{Alias, Expr, MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder};

pub struct Query;

impl Query {
    pub async fn find_player_by_id(db: &DbConn, id: u64) -> Result<Option<player_state::Model>, DbErr> {
        PlayerState::find_by_id(id).one(db).await
    }

    pub async fn find_plyer_by_ids(db: &DbConn, ids: Vec<u64>) -> Result<Vec<player_state::Model>, DbErr> {
        PlayerState::find()
            .filter(player_state::Column::EntityId.is_in(ids))
            .all(db)
            .await
    }

    /// If ok, returns (post models, num pages).
    pub async fn find_players(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
    ) -> Result<(Vec<player_state::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let mut filterQuery = PlayerState::find()
            .order_by_asc(player_state::Column::EntityId);
        if let Some(search) = search {
            filterQuery = filterQuery.filter(player_state::Column::Username.contains(&search));
        }

        let paginator = filterQuery.paginate(db, per_page);

        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    /// If ok, returns (post models, num pages).
    pub async fn find_locations(
        db: &DbConn,
        page: u64,
        posts_per_page: u64,
    ) -> Result<(Vec<location::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let paginator = Location::find()
            .order_by_asc(location::Column::EntityId)
            .paginate(db, posts_per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    /// If ok, returns (post models, num pages).
    pub async fn find_items(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
    ) -> Result<(Vec<item::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let mut filterQuery = Item::find()
            .order_by_asc(item::Column::Id);

        if let Some(search) = search {
            filterQuery = filterQuery.filter(item::Column::Name.contains(&search));
        }

        let paginator = filterQuery.paginate(db, per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn find_unique_item_tags(
        db: &DbConn,
    ) -> Result<Vec<String>, DbErr> {
        let items = dbg!(Item::find().distinct_on([item::Column::Tag]).build(db.get_database_backend()).to_string());
        let items = Item::find().select_only().column(item::Column::Tag).group_by(item::Column::Tag).order_by_asc(item::Column::Tag).into_model::<ItemTag>().all(db).await?;
        Ok(items.into_iter().map(|item| item.tag).collect())
    }

    pub async fn find_unique_item_tiers(
        db: &DbConn,
    ) -> Result<Vec<i32>, DbErr> {
        let items = dbg!(Item::find().distinct_on([item::Column::Tier]).build(db.get_database_backend()).to_string());
        let items = Item::find().select_only().column(item::Column::Tier).group_by(item::Column::Tier).order_by_asc(item::Column::Tier).into_model::<ItemTier>().all(db).await?;
        Ok(items.into_iter().map(|item| item.tier).collect())
    }


    /// If ok, returns (post models, num pages).
    pub async fn find_claim_descriptions(
        db: &DbConn,
        page: u64,
        per_page: u64,
        search: Option<String>,
    ) -> Result<(Vec<claim_description::Model>, ItemsAndPagesNumber), DbErr> {
        // Setup paginator
        let mut filterQuery = ClaimDescription::find()
            .order_by_asc(claim_description::Column::EntityId);

        if let Some(search) = search {
            filterQuery = filterQuery.filter(claim_description::Column::Name.contains(&search));
        }

        let paginator = filterQuery.paginate(db, per_page);
        let num_pages = paginator.num_items_and_pages().await?;

        // Fetch paginated posts
        paginator.fetch_page(page - 1).await.map(|p| (p, num_pages))
    }

    pub async fn find_claim_description_by_id(db: &DbConn, id: u64) -> Result<Option<claim_description::Model>, DbErr> {
        ClaimDescription::find_by_id(id).one(db).await
    }

    pub async fn skill_descriptions(db: &DbConn) -> Result<Vec<skill_desc::Model>, DbErr> {
        skill_desc::Entity::find().all(db).await
    }

    pub async fn full_experience_states_by_skill_id(db: &DbConn, skill_id: u64) -> Result<Vec<experience_state::Model>, DbErr> {
        experience_state::Entity::find()
            .order_by_asc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .all(db)
            .await
    }

    pub async fn get_experience_state_top_100_by_skill_id(db: &DbConn, skill_id: u64) -> Result<Vec<experience_state::Model>, DbErr> {
        experience_state::Entity::find()
            .order_by_desc(experience_state::Column::Experience)
            .filter(experience_state::Column::SkillId.eq(skill_id))
            .limit(100)
            .all(db)
            .await
    }

    pub async fn get_experience_state_top_100_total_level(db: &DbConn,level_case_sql: String) -> Result<Vec<(u64, i32)>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust(level_case_sql), Alias::new("level"))
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("level"), Order::Desc)
            .limit(100).to_owned();

        let query = match db.get_database_backend() {
            DbBackend::MySql => query.to_string(MysqlQueryBuilder),
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            DbBackend::Sqlite => query.to_string(SqliteQueryBuilder),
            _ => panic!("Unsupported database backend"),
        };

        Ok(db.query_all(Statement::from_string(db.get_database_backend(), query)).await?.into_iter().map(|row| {
            let level: Decimal = row.try_get("", "level").unwrap();
            let entity_id: u64 = row.try_get("","entity_id").unwrap();
            (entity_id, level.try_into().unwrap())
        }).collect::<Vec<(u64, i32)>>())
    }


    pub async fn get_experience_state_top_100_total_experience(db: &DbConn) -> Result<Vec<experience_state::Model>, DbErr> {
        let query = sea_orm::sea_query::Query::select()
            .column(experience_state::Column::EntityId)
            .expr_as(Expr::cust("sum(experience)"), Alias::new("total_experience"))
            .from(experience_state::Entity)
            .group_by_col(experience_state::Column::EntityId)
            .order_by_expr(Expr::cust("total_experience"), Order::Desc)
            .limit(100).to_owned();

        let query = match db.get_database_backend() {
            DbBackend::MySql => query.to_string(MysqlQueryBuilder),
            DbBackend::Postgres => query.to_string(PostgresQueryBuilder),
            DbBackend::Sqlite => query.to_string(SqliteQueryBuilder),
            _ => panic!("Unsupported database backend"),
        };

        Ok(db.query_all(Statement::from_string(db.get_database_backend(), query)).await?.into_iter().map(|row| {
            let entity_id: u64 = row.try_get("","entity_id").unwrap();
            let total_experience: Decimal = row.try_get("", "total_experience").unwrap();

            dbg!(&total_experience);

            experience_state::Model {
                entity_id,
                experience: total_experience.try_into().unwrap(),
                skill_id: 1,
            }

        }).collect())
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

