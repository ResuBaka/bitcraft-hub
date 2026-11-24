//! Only the fields the player overview needs from the game's placeable tables.
macro_rules! model {
    ($table:ident, $name:literal, $pk:ident: $key:ty, $($field:ident: $ty:ty),+) => {
        pub mod $table {
            use sea_orm::entity::prelude::*;
            #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
            #[sea_orm(table_name = $name)]
            pub struct Model {
                #[sea_orm(primary_key, auto_increment = false)]
                pub $pk: $key,
                $(pub $field: $ty),+
            }
            #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
            pub enum Relation {}
            impl ActiveModelBehavior for ActiveModel {}
        }
    }
}
model!(placeable_state, "placeable_state", entity_id: i64, region: i16, owner_entity_id: i64, placeable_id: i32);
model!(placeable_growth_timer, "placeable_growth_timer", entity_id: i64, region: i16, ends_at_ms: Option<i64>);
model!(placeable_desc, "placeable_desc", id: i32, name: String, tier: i32);
model!(placeable_interaction_desc, "placeable_interaction_desc", id: i32, placeable_id: i32, verb_phrase: String);
