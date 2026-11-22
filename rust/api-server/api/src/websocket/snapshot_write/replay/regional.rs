use super::*;
use entity::{
    building_state, deployable_state, experience_state, progressive_action_state,
    traveler_task_state,
};
use entity::{extraction_recipe_desc, player_state};

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn xp_rows() {
    use experience_state::Column::*;
    let ddl = "CREATE TABLE experience_state (entity_id bigint, skill_id integer, experience integer NOT NULL,
        region smallint NOT NULL, PRIMARY KEY(entity_id,skill_id))";
    replay::<experience_state::Entity>(
        "experience_state",
        ddl,
        OnConflict::columns([EntityId, SkillId])
            .update_columns([Experience, Region])
            .to_owned(),
        true,
        3000,
        |i, pass| experience_state::Model {
            entity_id: (1i64 << 40) + i64::from(i / 17),
            skill_id: i % 17,
            experience: i + i32::from(pass != Pass::Insert),
            region: if pass == Pass::Insert { 7 } else { 8 },
        },
        Writer::Fixed(experience),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn traveler_rows() {
    use traveler_task_state::Column::*;
    let ddl = "CREATE TABLE traveler_task_state (entity_id bigint PRIMARY KEY, player_entity_id bigint NOT NULL,
        traveler_id integer NOT NULL, task_id integer NOT NULL, completed boolean NOT NULL, region smallint NOT NULL)";
    replay::<traveler_task_state::Entity>(
        "traveler_task_state",
        ddl,
        OnConflict::column(EntityId)
            .update_columns([PlayerEntityId, TravelerId, TaskId, Completed, Region])
            .to_owned(),
        true,
        6000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            traveler_task_state::Model {
                entity_id: i.into(),
                player_entity_id: (1i64 << 40) + i64::from(v),
                traveler_id: v % 53,
                task_id: -v,
                completed: v % 2 == 0,
                region: if pass == Pass::Insert { 7 } else { 8 },
            }
        },
        Writer::Fixed(traveler_tasks),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn building_rows() {
    use building_state::Column::*;
    let ddl = "CREATE TABLE building_state (entity_id bigint PRIMARY KEY, claim_entity_id bigint NOT NULL,
        direction_index integer NOT NULL, building_description_id integer NOT NULL, constructed_by_player_entity_id bigint NOT NULL, region smallint NOT NULL)";
    replay::<building_state::Entity>(
        "building_state",
        ddl,
        OnConflict::column(EntityId)
            .update_columns([
                ClaimEntityId,
                DirectionIndex,
                BuildingDescriptionId,
                ConstructedByPlayerEntityId,
                Region,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            building_state::Model {
                entity_id: i.into(),
                claim_entity_id: (1i64 << 40) + i64::from(v),
                direction_index: v % 6,
                building_description_id: v * 2,
                constructed_by_player_entity_id: (1i64 << 42) - i64::from(v),
                region: if pass == Pass::Insert { 7 } else { 8 },
            }
        },
        Writer::Fixed(buildings),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn deployable_rows() {
    use deployable_state::Column::*;
    let ddl = "CREATE TABLE deployable_state (entity_id bigint PRIMARY KEY, owner_id bigint NOT NULL, claim_entity_id bigint NOT NULL,
        direction integer NOT NULL, deployable_description_id integer NOT NULL, nickname varchar NOT NULL, hidden boolean NOT NULL,
        appearance_override_id integer NOT NULL, region smallint NOT NULL)";
    replay::<deployable_state::Entity>(
        "deployable_state",
        ddl,
        OnConflict::column(EntityId)
            .update_columns([
                OwnerId,
                ClaimEntityId,
                Direction,
                DeployableDescriptionId,
                Nickname,
                Hidden,
                Region,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            deployable_state::Model {
                entity_id: i.into(),
                owner_id: (1i64 << 40) + i64::from(v),
                claim_entity_id: (1i64 << 43) - i64::from(v),
                direction: v % 6,
                deployable_description_id: v * 3,
                nickname: if i == 2 {
                    "猫".repeat(350_000)
                } else {
                    format!("cart '🐈' {v}")
                },
                hidden: v % 2 == 0,
                appearance_override_id: if pass == Pass::Update { -1 } else { i + 17 },
                region: if pass == Pass::Insert { 7 } else { 8 },
            }
        },
        deployable_writer(),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn progressive_rows() {
    use progressive_action_state::Column::*;
    let ddl = "CREATE TABLE progressive_action_state (entity_id bigint PRIMARY KEY, building_entity_id bigint NOT NULL, function_type integer NOT NULL,
        progress integer NOT NULL, recipe_id integer NOT NULL, craft_count integer NOT NULL, last_crit_outcome integer NOT NULL,
        owner_entity_id bigint NOT NULL, lock_expiration jsonb NOT NULL, preparation boolean NOT NULL, region smallint NOT NULL)";
    replay::<progressive_action_state::Entity>(
        "progressive_action_state",
        ddl,
        OnConflict::column(EntityId)
            .update_columns([
                BuildingEntityId,
                FunctionType,
                Progress,
                RecipeId,
                CraftCount,
                LastCritOutcome,
                OwnerEntityId,
                LockExpiration,
                Preparation,
                Region,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            progressive_action_state::Model {
                entity_id: i.into(),
                building_entity_id: (1i64 << 40) + i64::from(v),
                function_type: v % 5,
                progress: v,
                recipe_id: v * 2,
                craft_count: v * 3,
                last_crit_outcome: -v,
                owner_entity_id: (1i64 << 43) - i64::from(v),
                lock_expiration: entity::shared::timestamp::Timestamp {
                    __timestamp_micros_since_unix_epoch__: chrono::DateTime::from_timestamp_micros(
                        -123_456 + i64::from(v),
                    )
                    .unwrap(),
                },
                preparation: v % 2 == 0,
                region: if pass == Pass::Insert { 7 } else { 8 },
            }
        },
        Writer::Fixed(progressive_actions),
    )
    .await;
}
#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn player_rows() {
    use player_state::Column as C;
    let ddl = "CREATE TABLE player_state (entity_id bigint PRIMARY KEY, time_played integer NOT NULL,
        session_start_timestamp integer NOT NULL, time_signed_in integer NOT NULL, sign_in_timestamp integer NOT NULL,
        signed_in boolean NOT NULL, teleport_location json NOT NULL, traveler_tasks_expiration integer NOT NULL, region smallint NOT NULL)";
    replay::<player_state::Entity>(
        "player_state",
        ddl,
        OnConflict::column(C::EntityId)
            .update_columns([
                C::TimePlayed,
                C::SessionStartTimestamp,
                C::TimeSignedIn,
                C::SignInTimestamp,
                C::SignedIn,
                C::TeleportLocation,
                C::TravelerTasksExpiration,
                C::Region,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            player_state::Model {
                entity_id: (1i64 << 40) + i64::from(i),
                time_played: v * 2,
                session_start_timestamp: 10 + v,
                time_signed_in: v,
                sign_in_timestamp: 20 + v,
                signed_in: v % 2 == 0,
                traveler_tasks_expiration: 30 - v,
                region: if pass == Pass::Insert { 7 } else { 8 },
                teleport_location: player_state::TeleportLocation {
                    location: player_state::OffsetCoordinatesSmallMessage {
                        x: -v,
                        z: v,
                        dimension: v as u32 + 1,
                    },
                    location_type: if v % 2 == 0 {
                        player_state::TeleportLocationType::CustomLocation
                    } else {
                        player_state::TeleportLocationType::Waystone
                    },
                },
            }
        },
        Writer::Fixed(players),
    )
    .await;
}

#[tokio::test]
#[ignore = "requires SNAPSHOT_WRITER_TEST_URL"]
async fn extraction_rows() {
    use entity::{
        crafting_recipe::ToolRequirement,
        inventory::ItemType,
        shared::{
            experience_stack_f32::ExperienceStackF32, item_stack::ItemStack,
            probabilistic_item_stack::ProbabilisticItemStack,
        },
    };
    use extraction_recipe_desc::Column::*;
    let ddl = "CREATE TABLE extraction_recipe_desc (id integer PRIMARY KEY, resource_id integer NOT NULL, extracted_item_stacks json NOT NULL,
        tool_requirements json NOT NULL, allow_use_hands boolean NOT NULL, time_requirement real NOT NULL, stamina_requirement real NOT NULL,
        cargo_id integer, experience_per_progress jsonb)";
    replay::<extraction_recipe_desc::Entity>(
        "extraction_recipe_desc",
        ddl,
        OnConflict::column(Id)
            .update_columns([
                ResourceId,
                ExtractedItemStacks,
                ToolRequirements,
                AllowUseHands,
                TimeRequirement,
                StaminaRequirement,
                CargoId,
                ExperiencePerProgress,
            ])
            .to_owned(),
        true,
        3000,
        |i, pass| {
            let v = i + i32::from(pass != Pass::Insert);
            extraction_recipe_desc::Model {
                id: i,
                resource_id: -v,
                extracted_item_stacks: vec![ProbabilisticItemStack {
                    item_stack: (v % 2 == 0).then_some(ItemStack {
                        item_id: v,
                        quantity: v + 1,
                        item_type: ItemType::Cargo,
                        durability: None,
                    }),
                    probability: 0.1,
                }],
                tool_requirements: vec![ToolRequirement {
                    tool_type: v,
                    level: 1,
                    power: 2,
                }],
                allow_use_hands: v % 2 == 0,
                time_requirement: v as f32 + 0.5,
                stamina_requirement: v as f32 + 0.25,
                cargo_id: match v % 3 {
                    0 => None,
                    1 => Some(0),
                    _ => Some(v),
                },
                experience_per_progress: match v % 3 {
                    0 => None,
                    1 => Some(vec![]),
                    _ => Some(vec![ExperienceStackF32 {
                        skill_id: v,
                        quantity: 2.7,
                    }]),
                },
            }
        },
        extraction_writer(),
    )
    .await;
}
