use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(
                Table::truncate()
                    .table(BuildingNicknameState::Table)
                    .to_owned(),
            )
            .await?;
        if !manager
            .has_column(
                BuildingNicknameState::Table.to_string(),
                BuildingNicknameState::Region.to_string(),
            )
            .await?
        {
            let building_nickname_state_alter = Table::alter()
                .table(BuildingNicknameState::Table)
                .add_column(string(BuildingNicknameState::Region))
                .to_owned();
            manager.alter_table(building_nickname_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("BuildingNicknameStateRegionIndex")
                    .table(BuildingNicknameState::Table)
                    .col(BuildingNicknameState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(BuildingState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                BuildingState::Table.to_string(),
                BuildingState::Region.to_string(),
            )
            .await?
        {
            let building_state_alter = Table::alter()
                .table(BuildingState::Table)
                .add_column(string(BuildingState::Region))
                .to_owned();
            manager.alter_table(building_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("BuildingStateRegionIndex")
                    .table(BuildingState::Table)
                    .col(BuildingState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(ClaimLocalState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                ClaimLocalState::Table.to_string(),
                ClaimLocalState::Region.to_string(),
            )
            .await?
        {
            let claim_local_state_alter = Table::alter()
                .table(ClaimLocalState::Table)
                .add_column(string(ClaimLocalState::Region))
                .to_owned();
            manager.alter_table(claim_local_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("ClaimLocalStateRegionIndex")
                    .table(ClaimLocalState::Table)
                    .col(ClaimLocalState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(ClaimMemberState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                ClaimMemberState::Table.to_string(),
                ClaimMemberState::Region.to_string(),
            )
            .await?
        {
            let claim_member_local_state_alter = Table::alter()
                .table(ClaimMemberState::Table)
                .add_column(string(ClaimMemberState::Region))
                .to_owned();
            manager.alter_table(claim_member_local_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("ClaimMemberStateRegionIndex")
                    .table(ClaimMemberState::Table)
                    .col(ClaimMemberState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(ClaimState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                ClaimState::Table.to_string(),
                ClaimState::Region.to_string(),
            )
            .await?
        {
            let claim_local_state_alter = Table::alter()
                .table(ClaimState::Table)
                .add_column(string(ClaimState::Region))
                .to_owned();
            manager.alter_table(claim_local_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("ClaimStateRegionIndex")
                    .table(ClaimState::Table)
                    .col(ClaimState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(ClaimTechState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                ClaimTechState::Table.to_string(),
                ClaimTechState::Region.to_string(),
            )
            .await?
        {
            let claim_tech_state_alter = Table::alter()
                .table(ClaimTechState::Table)
                .add_column(string(ClaimTechState::Region))
                .to_owned();
            manager.alter_table(claim_tech_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("ClaimTechStateRegionIndex")
                    .table(ClaimTechState::Table)
                    .col(ClaimTechState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(DeployableState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                DeployableState::Table.to_string(),
                DeployableState::Region.to_string(),
            )
            .await?
        {
            let deployable_state_alter = Table::alter()
                .table(DeployableState::Table)
                .add_column(string(DeployableState::Region))
                .to_owned();
            manager.alter_table(deployable_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("DeployableStateRegionIndex")
                    .table(DeployableState::Table)
                    .col(DeployableState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(ExperienceState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                ExperienceState::Table.to_string(),
                ExperienceState::Region.to_string(),
            )
            .await?
        {
            let experience_state_alter = Table::alter()
                .table(ExperienceState::Table)
                .add_column(string(ExperienceState::Region))
                .to_owned();
            manager.alter_table(experience_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("ExperienceStateRegionIndex")
                    .table(ExperienceState::Table)
                    .col(ExperienceState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(PlayerState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                PlayerState::Table.to_string(),
                PlayerState::Region.to_string(),
            )
            .await?
        {
            let player_state_alter = Table::alter()
                .table(PlayerState::Table)
                .add_column(string(PlayerState::Region))
                .to_owned();
            manager.alter_table(player_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("PlayerStateRegionIndex")
                    .table(PlayerState::Table)
                    .col(PlayerState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(
                Table::truncate()
                    .table(PlayerUsernameState::Table)
                    .to_owned(),
            )
            .await?;
        if !manager
            .has_column(
                PlayerUsernameState::Table.to_string(),
                PlayerUsernameState::Region.to_string(),
            )
            .await?
        {
            let player_username_state_alter = Table::alter()
                .table(PlayerUsernameState::Table)
                .add_column(string(PlayerUsernameState::Region))
                .to_owned();
            manager.alter_table(player_username_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("PlayerUsernameStateRegionIndex")
                    .table(PlayerUsernameState::Table)
                    .col(PlayerUsernameState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(VaultState::Table).to_owned())
            .await?;
        if !manager
            .has_column(
                VaultState::Table.to_string(),
                VaultState::Region.to_string(),
            )
            .await?
        {
            let vault_state_alter = Table::alter()
                .table(VaultState::Table)
                .add_column(string(VaultState::Region))
                .to_owned();
            manager.alter_table(vault_state_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("VaultStateRegionIndex")
                    .table(VaultState::Table)
                    .col(VaultState::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(
                Table::truncate()
                    .table(VaultStateCollectibles::Table)
                    .to_owned(),
            )
            .await?;
        if !manager
            .has_column(
                VaultStateCollectibles::Table.to_string(),
                VaultStateCollectibles::Region.to_string(),
            )
            .await?
        {
            let vault_state_collectible_alter = Table::alter()
                .table(VaultStateCollectibles::Table)
                .add_column(string(VaultStateCollectibles::Region))
                .to_owned();
            manager.alter_table(vault_state_collectible_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("VaultStateCollectiblesRegionIndex")
                    .table(VaultStateCollectibles::Table)
                    .col(VaultStateCollectibles::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        manager
            .truncate_table(Table::truncate().table(Inventory::Table).to_owned())
            .await?;
        if !manager
            .has_column(Inventory::Table.to_string(), Inventory::Region.to_string())
            .await?
        {
            let inventory_alter = Table::alter()
                .table(Inventory::Table)
                .add_column(string(Inventory::Region))
                .to_owned();
            manager.alter_table(inventory_alter).await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("InventoryRegionIndex")
                    .table(Inventory::Table)
                    .col(Inventory::Region)
                    .to_owned(),
            )
            .await
            .expect("Couldn't create index");

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let building_nickname_state_alter = Table::alter()
            .table(BuildingNicknameState::Table)
            .drop_column(BuildingNicknameState::Region)
            .to_owned();
        manager.alter_table(building_nickname_state_alter).await?;

        let building_state_alter = Table::alter()
            .table(BuildingState::Table)
            .drop_column(BuildingState::Region)
            .to_owned();
        manager.alter_table(building_state_alter).await?;

        let claim_local_state_alter = Table::alter()
            .table(ClaimLocalState::Table)
            .drop_column(ClaimLocalState::Region)
            .to_owned();
        manager.alter_table(claim_local_state_alter).await?;

        let claim_member_local_state_alter = Table::alter()
            .table(ClaimMemberState::Table)
            .drop_column(ClaimMemberState::Region)
            .to_owned();
        manager.alter_table(claim_member_local_state_alter).await?;

        let claim_local_state_alter = Table::alter()
            .table(ClaimState::Table)
            .drop_column(ClaimState::Region)
            .to_owned();
        manager.alter_table(claim_local_state_alter).await?;

        let claim_tech_state_alter = Table::alter()
            .table(ClaimTechState::Table)
            .drop_column(ClaimTechState::Region)
            .to_owned();
        manager.alter_table(claim_tech_state_alter).await?;

        let deployable_state_alter = Table::alter()
            .table(DeployableState::Table)
            .drop_column(DeployableState::Region)
            .to_owned();
        manager.alter_table(deployable_state_alter).await?;

        let experience_state_alter = Table::alter()
            .table(ExperienceState::Table)
            .drop_column(ExperienceState::Region)
            .to_owned();
        manager.alter_table(experience_state_alter).await?;

        let player_state_alter = Table::alter()
            .table(PlayerState::Table)
            .drop_column(PlayerState::Region)
            .to_owned();
        manager.alter_table(player_state_alter).await?;

        let player_username_state_alter = Table::alter()
            .table(PlayerUsernameState::Table)
            .drop_column(PlayerUsernameState::Region)
            .to_owned();
        manager.alter_table(player_username_state_alter).await?;

        let vault_state_alter = Table::alter()
            .table(VaultState::Table)
            .drop_column(VaultState::Region)
            .to_owned();
        manager.alter_table(vault_state_alter).await?;

        let vault_state_collectible_alter = Table::alter()
            .table(VaultStateCollectibles::Table)
            .drop_column(VaultStateCollectibles::Region)
            .to_owned();
        manager.alter_table(vault_state_collectible_alter).await?;

        let inventory_alter = Table::alter()
            .table(Inventory::Table)
            .drop_column(Inventory::Region)
            .to_owned();
        manager.alter_table(inventory_alter).await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum BuildingNicknameState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum BuildingState {
    Table,
    Region,
}

#[derive(DeriveIden)]
enum ClaimLocalState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum ClaimMemberState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum ClaimState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum ClaimTechState {
    Table,
    Region,
}

#[derive(DeriveIden)]
enum DeployableState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum ExperienceState {
    Table,
    Region,
}

#[derive(DeriveIden)]
enum PlayerState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum PlayerUsernameState {
    Table,
    Region,
}

#[derive(DeriveIden)]
enum VaultState {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum VaultStateCollectibles {
    Table,
    Region,
}
#[derive(DeriveIden)]
enum Inventory {
    Table,
    Region,
}
