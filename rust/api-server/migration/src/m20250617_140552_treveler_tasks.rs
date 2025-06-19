use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Schema;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let builder = manager.get_database_backend();
        let db = manager.get_connection();
        let schema = Schema::new(builder);

        db.execute(
            builder.build(&schema.create_table_from_entity(entity::traveler_task_desc::Entity)),
        )
        .await?;
        db.execute(
            builder.build(&schema.create_table_from_entity(entity::traveler_task_state::Entity)),
        )
        .await?;
        db.execute(builder.build(&schema.create_table_from_entity(entity::npc_desc::Entity)))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(entity::traveler_task_desc::Entity)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(entity::traveler_task_state::Entity)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(entity::npc_desc::Entity).to_owned())
            .await
    }
}
