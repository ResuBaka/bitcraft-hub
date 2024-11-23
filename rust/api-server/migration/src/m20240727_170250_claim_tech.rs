use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SkillDesc::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillDesc::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillDesc::Skill).integer().not_null())
                    .col(ColumnDef::new(SkillDesc::Name).string().not_null())
                    .col(ColumnDef::new(SkillDesc::Description).string().not_null())
                    .col(ColumnDef::new(SkillDesc::IconAssetName).string().not_null())
                    .col(ColumnDef::new(SkillDesc::Title).string().not_null())
                    .col(ColumnDef::new(SkillDesc::SkillCategory).integer().not_null())
                    .to_owned(),
            )
            .await
            .expect("Creating ClaimTechState table");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SkillDesc::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum SkillDesc {
    Table,
    Id,
    Skill,
    Name,
    Description,
    IconAssetName,
    Title,
    SkillCategory,
}
