use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        manager
            .create_table(
                Table::create()
                    .table(ExperienceStateChanges::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ExperienceStateChanges::Created)
                            .timestamp_with_time_zone()
                            .default(SimpleExpr::Custom("now()".into()))
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExperienceStateChanges::EntityId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExperienceStateChanges::SkillId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ExperienceStateChanges::Experience)
                            .integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(ExperienceStateChanges::Created)
                            .col(ExperienceStateChanges::EntityId)
                            .col(ExperienceStateChanges::SkillId),
                    )
                    .to_owned(),
            )
            .await?;

        let stmt = Statement::from_string(
            manager.get_database_backend(),
            format!(
                "SELECT create_hypertable('{}', by_range('created'));",
                match ExperienceStateChanges::Table.into_table_ref() {
                    TableRef::Table(table_ref) => table_ref.to_string(),
                    _ => panic!("Unexpected table ref type"),
                }
            )
            .as_str(),
        );

        db.execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ExperienceStateChanges::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum ExperienceStateChanges {
    Table,
    Created,
    EntityId,
    SkillId,
    Experience,
}
