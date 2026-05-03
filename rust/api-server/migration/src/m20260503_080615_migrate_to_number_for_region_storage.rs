use crate::sea_orm::Statement;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table_name in REGION_TABLES {
            migrate_region_to_smallint(manager, table_name).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table_name in REGION_TABLES {
            migrate_region_to_text(manager, table_name).await?;
        }

        Ok(())
    }
}

const REGION_TABLES: &[&str] = &[
    "building_nickname_state",
    "building_state",
    "claim_local_state",
    "claim_member_state",
    "claim_state",
    "claim_tech_state",
    "deployable_state",
    "dimension_description_state",
    "experience_state",
    "interior_network_desc",
    "inventory",
    "location_state",
    "permission_state",
    "player_housing_state",
    "player_state",
    "player_to_claim",
    "player_username_state",
    "portal_state",
    "progressive_action_state",
    "trade_order",
    "traveler_task_state",
    "user_state",
    "vault_state",
    "vault_state_collectibles",
];

async fn migrate_region_to_smallint(
    manager: &SchemaManager<'_>,
    table_name: &str,
) -> Result<(), DbErr> {
    if !manager
        .has_column(table_name.to_owned(), "region".to_owned())
        .await?
    {
        tracing::info!("Skipping table {} because it doesn't exist", table_name);
        return Ok(());
    }

    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                ALTER TABLE {table_name}
                ALTER COLUMN region DROP DEFAULT
                "#,
            ),
        ))
        .await?;

    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                ALTER TABLE {table_name}
                ALTER COLUMN region TYPE SMALLINT
                USING substring(region from '([0-9]+)$')::smallint
                "#,
            ),
        ))
        .await?;

    Ok(())
}

async fn migrate_region_to_text(
    manager: &SchemaManager<'_>,
    table_name: &str,
) -> Result<(), DbErr> {
    if !manager
        .has_column(table_name.to_owned(), "region".to_owned())
        .await?
    {
        tracing::info!("Skipping table {} because it doesn't exist", table_name);
        return Ok(());
    }

    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                ALTER TABLE {table_name}
                ALTER COLUMN region DROP DEFAULT
                "#,
            ),
        ))
        .await?;

    manager
        .get_connection()
        .execute(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                ALTER TABLE {table_name}
                ALTER COLUMN region TYPE TEXT
                USING ('bitcraft-live-' || region::text)
                "#,
            ),
        ))
        .await?;

    Ok(())
}
