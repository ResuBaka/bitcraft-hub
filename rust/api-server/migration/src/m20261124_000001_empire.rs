use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(include_str!("m20261124_000001_empire.sql"))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(
            "DROP TABLE empire_state, empire_player_data_state, empire_rank_state,
             empire_settlement_state, empire_node_state, empire_chunk_state, empire_node_siege_state,
             empire_emblem_state, empire_directive_state, empire_foundry_state, empire_siege_engine_state,
             empire_notification_state, empire_rank_desc, empire_color_desc, empire_icon_desc,
             empire_supplies_desc, empire_territory_desc, empire_notification_desc"
        ).await?;
        Ok(())
    }
}
