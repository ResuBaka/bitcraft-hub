use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("extraction_recipe_desc"))
                    .add_column(ColumnDef::new(Alias::new("cargo_id")).integer().null())
                    .add_column(
                        ColumnDef::new(Alias::new("experience_per_progress"))
                            .json_binary()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("extraction_recipe_desc"))
                    .drop_column(Alias::new("cargo_id"))
                    .drop_column(Alias::new("experience_per_progress"))
                    .to_owned(),
            )
            .await
    }
}
