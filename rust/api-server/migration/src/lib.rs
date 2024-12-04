pub use sea_orm_migration::prelude::*;

mod m20220101_000001_player_state;
mod m20240727_170250_skill_desc;
mod m20240728_160123_vehicle_state;
mod m20240801_163734_changes_experience_state;
mod m20241201_181644_fix_typo_for_player_state;
mod m20241203_212013_vault_state;
mod m20241204_132147_collectible_desc;

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
        ]
    }
}
