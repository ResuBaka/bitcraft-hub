pub use sea_orm_migration::prelude::*;

mod m20220101_000001_player_state;
mod m20240727_170250_skill_desc;
mod m20240728_160123_vehicle_state;
mod m20240801_163734_changes_experience_state;
mod m20241201_181644_fix_typo_for_player_state;
mod m20241203_212013_vault_state;
mod m20241204_132147_collectible_desc;
mod m20241208_112015_inventory_changelog;
mod m20241208_205237_raw_event_data;
mod m20241210_182840_raw_event_data_user_id;
mod m20250219_215440_add_to_item_desc_convert_to_on_durability_zero;
mod m20250219_221732_add_to_building_desc_interact_permission;
mod m20250220_115114_add_crafting_recipe;
mod m20250221_122758_add_new_fields_to_collectible_desc;
mod m20250221_220058_player_to_claim;


pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_player_state::Migration),
            Box::new(m20240727_170250_skill_desc::Migration),
            Box::new(m20240728_160123_vehicle_state::Migration),
            Box::new(m20240801_163734_changes_experience_state::Migration),
            Box::new(m20241201_181644_fix_typo_for_player_state::Migration),
            Box::new(m20241203_212013_vault_state::Migration),
            Box::new(m20241204_132147_collectible_desc::Migration),
            Box::new(m20241208_112015_inventory_changelog::Migration),
            Box::new(m20241208_205237_raw_event_data::Migration),
            Box::new(m20241210_182840_raw_event_data_user_id::Migration),
            Box::new(m20250219_215440_add_to_item_desc_convert_to_on_durability_zero::Migration),
            Box::new(m20250219_221732_add_to_building_desc_interact_permission::Migration),
            Box::new(m20250220_115114_add_crafting_recipe::Migration),
            Box::new(m20250221_220058_player_to_claim::Migration),
        ]
    }
}
