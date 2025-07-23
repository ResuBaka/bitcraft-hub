use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut player_state_down = Table::alter()
            .table(CargoDesc::Table)
            .drop_column(CargoDesc::OnDestroyYieldCargos)
            .to_owned();

        manager.alter_table(player_state_down).await?;

        player_state_down = Table::alter()
            .table(CargoDesc::Table)
            .add_column(
                integer(CargoDesc::OnDestroyYieldCargos)
                    .array(ColumnType::Integer)
                    .default(Vec::<i32>::new()),
            )
            .to_owned();
        manager.alter_table(player_state_down).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut player_state_down = Table::alter()
            .table(CargoDesc::Table)
            .drop_column(CargoDesc::OnDestroyYieldCargos)
            .to_owned();

        manager.alter_table(player_state_down).await?;

        player_state_down = Table::alter()
            .table(CargoDesc::Table)
            .add_column(integer(CargoDesc::OnDestroyYieldCargos).json().null())
            .to_owned();
        manager.alter_table(player_state_down).await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum CargoDesc {
    Table,
    OnDestroyYieldCargos,
}
