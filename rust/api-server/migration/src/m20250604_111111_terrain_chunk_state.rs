use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TerrainChunkState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TerrainChunkState::ChunkIndex)
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TerrainChunkState::ChunkX).integer().not_null())
                    .col(ColumnDef::new(TerrainChunkState::ChunkZ).integer().not_null())
                    .col(ColumnDef::new(TerrainChunkState::Dimension).integer().not_null())
                    .col(ColumnDef::new(TerrainChunkState::Biomes).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::BiomeDensity).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::Elevations).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::WaterLevels).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::WaterBodyTypes).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::ZoningTypes).json_binary().not_null())
                    .col(ColumnDef::new(TerrainChunkState::OriginalElevations).json_binary().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TerrainChunkState::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum TerrainChunkState {
    Table,
    ChunkIndex,
    ChunkX,
    ChunkZ,
    Dimension,
    Biomes,
    BiomeDensity,
    Elevations,
    WaterLevels,
    WaterBodyTypes,
    ZoningTypes,
    OriginalElevations,
}
