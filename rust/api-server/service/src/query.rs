use ::entity::{
    player_state, player_state::Entity as PlayerState,
    item, item::Entity as Item,
    location, location::Entity as Location
};
use sea_orm::*;

pub struct Query;

impl Query {
    pub async fn find_post_by_id(db: &DbConn, id: u64) -> Result<Option<player_state::Model>, DbErr> {
        PlayerState::find_by_id(id).one(db).await
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
}

#[derive(FromQueryResult)]
struct ItemTag {
    pub tag: String,
}

#[derive(FromQueryResult)]
struct ItemTier {
    pub tier: i32,
}

