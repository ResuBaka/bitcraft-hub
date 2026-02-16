use crate::sea_orm::{EntityName, Schema};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let builder = manager.get_database_backend();
        let db = manager.get_connection();
        let schema = Schema::new(builder);

        db.execute(
            builder.build(&schema.create_table_from_entity(entity::permission_state::Entity)),
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_table(entity::permission_state::Entity.table_name())
            .await?
        {
            manager
                .drop_table(
                    Table::drop()
                        .table(entity::permission_state::Entity)
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
