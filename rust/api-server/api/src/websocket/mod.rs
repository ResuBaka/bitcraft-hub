use crate::AppState;
use crate::config::Config;
use crate::leaderboard::experience_to_level;
use entity::{
    cargo_desc, claim_local_state, claim_member_state, claim_state, claim_tech_state, crafting_recipe, deployable_state, item_desc, mobile_entity_state, vault_state_collectibles
};
#[allow(unused_imports)]
use entity::{raw_event_data, skill_desc};
use game_module::module_bindings::*;
use migration::IdenList;
use sea_orm::{EntityTrait, IntoActiveModel, ModelTrait, sea_query};
use serde::{Deserialize, Serialize};
use spacetimedb_sdk::{Compression, DbContext, Error, Table, TableWithPrimaryKey, credentials};
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Instant;
use tokio::time::{Duration, sleep};

use serde_json::Value;
use time::OffsetDateTime;
use reqwest::ClientBuilder;
use reqwest_websocket::{Message, RequestBuilderExt, WebSocket};
use axum::http::HeaderMap;
use axum::http::header::SEC_WEBSOCKET_PROTOCOL;
use log::{debug, error, warn};
use futures::{SinkExt, TryStreamExt};

use std::fs::File;
use std::io::prelude::*;

fn connect_to_db(db_name: &str, db_host: &str) -> DbConnection {
    DbConnection::builder()
        // Register our `on_connect` callback, which will save our auth token.
        .on_connect(on_connected)
        // Register our `on_connect_error` callback, which will print a message, then exit the process.
        .on_connect_error(on_connect_error)
        // Our `on_disconnect` callback, which will print a message, then exit the process.
        .on_disconnect(on_disconnected)
        // If the user has previously connected, we'll have saved a token in the `on_connect` callback.
        // In that case, we'll load it and pass it to `with_token`,
        // so we can re-authenticate as the same `Identity`.
        .with_token(creds_store().load().expect("Error loading credentials"))
        // Set the database name we chose when we called `spacetime publish`.
        .with_module_name(db_name)
        // Set the URI of the SpacetimeDB host that's running our database.
        .with_uri(db_host)
        // Finalize configuration and connect!
        .with_compression(Compression::Brotli)
        .build()
        .expect("Failed to connect")
}

// /// Register subscriptions for all rows of both tables.
// fn subscribe_to_tables(ctx: &DbConnection) {
//     ctx.subscription_builder()
//         .on_applied(on_sub_applied)
//         .on_error(on_sub_error)
//         .subscribe(["SELECT * FROM player_state", "SELECT * FROM mobile_entity_state"]);
// }

/// Or `on_error` callback:
/// print the error, then exit the process.
fn on_sub_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Subscription failed: {}", err);
    // std::process::exit(1);
}

fn creds_store() -> credentials::File {
    credentials::File::new("bitcraft-beta")
}

/// Our `on_connect` callback: save our credentials to a file.
fn on_connected(_ctx: &DbConnection, _identity: spacetimedb_sdk::Identity, token: &str) {
    if let Err(e) = creds_store().save(token) {
        eprintln!("Failed to save credentials: {:?}", e);
    }
}

/// Our `on_connect_error` callback: print the error, then exit the process.
fn on_connect_error(_ctx: &ErrorContext, err: Error) {
    eprintln!("Connection error: {:?}", err);
    // std::process::exit(1);
}

/// Our `on_disconnect` callback: print a note, then exit the process.
fn on_disconnected(_ctx: &ErrorContext, err: Option<Error>) {
    if let Some(err) = err {
        eprintln!("Disconnected: {}", err);
        // std::process::exit(1);
    } else {
        println!("Disconnected.");
        // std::process::exit(0);
    }
}
fn connect_to_db_logic(
    config: &Config,
    database: &str,
    remove_desc: &bool,
    mobile_entity_state_tx: &UnboundedSender<SpacetimeUpdateMessages<MobileEntityState>>,
    player_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerState>>,
    player_username_state_tx: &UnboundedSender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    experience_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    inventory_state_tx: &UnboundedSender<SpacetimeUpdateMessages<InventoryState>>,
    item_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ItemDesc>>,
    cargo_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CargoDesc>>,
    vault_state_collectibles_tx: &UnboundedSender<SpacetimeUpdateMessages<VaultState>>,
    deployable_state_tx: &UnboundedSender<SpacetimeUpdateMessages<DeployableState>>,
    claim_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimState>>,
    claim_local_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimLocalState>>,
    claim_member_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimMemberState>>,
    skill_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<SkillDesc>>,
    claim_tech_state_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechState>>,
    claim_tech_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<ClaimTechDesc>>,
    crafting_recipe_desc_tx: &UnboundedSender<SpacetimeUpdateMessages<CraftingRecipeDesc>>,
) {
    let ctx = connect_to_db(database, config.spacetimedb_url().as_ref());
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db.mobile_entity_state().on_update(
        move |_ctx: &EventContext, old: &MobileEntityState, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db
        .mobile_entity_state()
        .on_insert(move |_ctx: &EventContext, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_mobile_entity_state_tx = mobile_entity_state_tx.clone();
    ctx.db
        .mobile_entity_state()
        .on_delete(move |_ctx: &EventContext, new: &MobileEntityState| {
            temp_mobile_entity_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_player_state_tx = player_state_tx.clone();
    ctx.db.player_state().on_update(
        move |_ctx: &EventContext, old: &PlayerState, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_player_state_tx = player_state_tx.clone();
    ctx.db
        .player_state()
        .on_insert(move |_ctx: &EventContext, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_player_state_tx = player_state_tx.clone();
    ctx.db
        .player_state()
        .on_delete(move |_ctx: &EventContext, new: &PlayerState| {
            temp_player_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_update(
        move |_ctx: &EventContext, old: &PlayerUsernameState, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_insert(
        move |_ctx: &EventContext, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        },
    );
    let temp_player_username_state_tx = player_username_state_tx.clone();
    ctx.db.player_username_state().on_delete(
        move |_ctx: &EventContext, new: &PlayerUsernameState| {
            temp_player_username_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        },
    );

    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db.experience_state().on_update(
        move |_ctx: &EventContext, old: &ExperienceState, new: &ExperienceState| {
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db
        .experience_state()
        .on_insert(move |_ctx: &EventContext, new: &ExperienceState| {
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_experience_state_tx = experience_state_tx.clone();
    ctx.db
        .experience_state()
        .on_delete(move |_ctx: &EventContext, new: &ExperienceState| {
            temp_experience_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_inventory_state_tx = inventory_state_tx.clone();
    ctx.db.inventory_state().on_update(
        move |_ctx: &EventContext, old: &InventoryState, new: &InventoryState| {
            temp_inventory_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_inventory_state_tx = inventory_state_tx.clone();
    ctx.db
        .inventory_state()
        .on_insert(move |_ctx: &EventContext, new: &InventoryState| {
            temp_inventory_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_inventory_state_tx = inventory_state_tx.clone();
    ctx.db
        .inventory_state()
        .on_delete(move |_ctx: &EventContext, new: &InventoryState| {
            temp_inventory_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_item_desc_tx = item_desc_tx.clone();
    ctx.db
        .item_desc()
        .on_update(move |_ctx: &EventContext, old: &ItemDesc, new: &ItemDesc| {
            temp_item_desc_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        });
    let temp_item_desc_tx = item_desc_tx.clone();
    ctx.db
        .item_desc()
        .on_insert(move |_ctx: &EventContext, new: &ItemDesc| {
            temp_item_desc_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_item_desc_tx = item_desc_tx.clone();
    ctx.db
        .item_desc()
        .on_delete(move |_ctx: &EventContext, new: &ItemDesc| {
            temp_item_desc_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_cargo_desc_tx = cargo_desc_tx.clone();
    ctx.db.cargo_desc().on_update(
        move |_ctx: &EventContext, old: &CargoDesc, new: &CargoDesc| {
            temp_cargo_desc_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_cargo_desc_tx = cargo_desc_tx.clone();
    ctx.db
        .cargo_desc()
        .on_insert(move |_ctx: &EventContext, new: &CargoDesc| {
            temp_cargo_desc_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_cargo_desc_tx = cargo_desc_tx.clone();
    ctx.db
        .cargo_desc()
        .on_delete(move |_ctx: &EventContext, new: &CargoDesc| {
            temp_cargo_desc_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
    ctx.db.vault_state().on_update(
        move |_ctx: &EventContext, old: &VaultState, new: &VaultState| {
            temp_vault_state_collectibles_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
    ctx.db
        .vault_state()
        .on_insert(move |_ctx: &EventContext, new: &VaultState| {
            temp_vault_state_collectibles_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_vault_state_collectibles_tx = vault_state_collectibles_tx.clone();
    ctx.db
        .vault_state()
        .on_delete(move |_ctx: &EventContext, new: &VaultState| {
            temp_vault_state_collectibles_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_claim_state_tx = claim_state_tx.clone();
    ctx.db.claim_state().on_update(
        move |_ctx: &EventContext, old: &ClaimState, new: &ClaimState| {
            temp_claim_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_claim_state_tx = claim_state_tx.clone();
    ctx.db
        .claim_state()
        .on_insert(move |_ctx: &EventContext, new: &ClaimState| {
            temp_claim_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_claim_state_tx = claim_state_tx.clone();
    ctx.db
        .claim_state()
        .on_delete(move |_ctx: &EventContext, new: &ClaimState| {
            temp_claim_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_deployable_state_tx = deployable_state_tx.clone();
    ctx.db.deployable_state().on_update(
        move |_ctx: &EventContext, old: &DeployableState, new: &DeployableState| {
            temp_deployable_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_deployable_state_tx = deployable_state_tx.clone();
    ctx.db
        .deployable_state()
        .on_insert(move |_ctx: &EventContext, new: &DeployableState| {
            temp_deployable_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_deployable_state_tx = deployable_state_tx.clone();
    ctx.db
        .deployable_state()
        .on_delete(move |_ctx: &EventContext, new: &DeployableState| {
            temp_deployable_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_claim_local_state_tx = claim_local_state_tx.clone();
    ctx.db.claim_local_state().on_update(
        move |_ctx: &EventContext, old: &ClaimLocalState, new: &ClaimLocalState| {
            temp_claim_local_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_claim_local_state_tx = claim_local_state_tx.clone();
    ctx.db
        .claim_local_state()
        .on_insert(move |_ctx: &EventContext, new: &ClaimLocalState| {
            temp_claim_local_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_claim_local_state_tx = claim_local_state_tx.clone();
    ctx.db
        .claim_local_state()
        .on_delete(move |_ctx: &EventContext, new: &ClaimLocalState| {
            temp_claim_local_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_claim_member_state_tx = claim_member_state_tx.clone();
    ctx.db.claim_member_state().on_update(
        move |_ctx: &EventContext, old: &ClaimMemberState, new: &ClaimMemberState| {
            temp_claim_member_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_claim_member_state_tx = claim_member_state_tx.clone();
    ctx.db
        .claim_member_state()
        .on_insert(move |_ctx: &EventContext, new: &ClaimMemberState| {
            temp_claim_member_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_claim_member_state_tx = claim_member_state_tx.clone();
    ctx.db
        .claim_member_state()
        .on_delete(move |_ctx: &EventContext, new: &ClaimMemberState| {
            temp_claim_member_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_skill_desc_tx = skill_desc_tx.clone();
    ctx.db.skill_desc().on_update(
        move |_ctx: &EventContext, old: &SkillDesc, new: &SkillDesc| {
            temp_skill_desc_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_skill_desc_tx = skill_desc_tx.clone();
    ctx.db
        .skill_desc()
        .on_insert(move |_ctx: &EventContext, new: &SkillDesc| {
            temp_skill_desc_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_skill_desc_tx = skill_desc_tx.clone();
    ctx.db
        .skill_desc()
        .on_delete(move |_ctx: &EventContext, new: &SkillDesc| {
            temp_skill_desc_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_claim_tech_state_tx = claim_tech_state_tx.clone();
    ctx.db.claim_tech_state().on_update(
        move |_ctx: &EventContext, old: &ClaimTechState, new: &ClaimTechState| {
            temp_claim_tech_state_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_claim_tech_state_tx = claim_tech_state_tx.clone();
    ctx.db
        .claim_tech_state()
        .on_insert(move |_ctx: &EventContext, new: &ClaimTechState| {
            temp_claim_tech_state_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_claim_tech_state_tx = claim_tech_state_tx.clone();
    ctx.db
        .claim_tech_state()
        .on_delete(move |_ctx: &EventContext, new: &ClaimTechState| {
            temp_claim_tech_state_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });

    let temp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
    ctx.db.claim_tech_desc().on_update(
        move |_ctx: &EventContext, old: &ClaimTechDesc, new: &ClaimTechDesc| {
            temp_claim_tech_desc_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
    ctx.db
        .claim_tech_desc()
        .on_insert(move |_ctx: &EventContext, new: &ClaimTechDesc| {
            temp_claim_tech_desc_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_claim_tech_desc_tx = claim_tech_desc_tx.clone();
    ctx.db
        .claim_tech_desc()
        .on_delete(move |_ctx: &EventContext, new: &ClaimTechDesc| {
            temp_claim_tech_desc_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });



    let temp_crafting_recipe_desc_tx_tx = crafting_recipe_desc_tx.clone();
    ctx.db.crafting_recipe_desc().on_update(
        move |_ctx: &EventContext, old: &CraftingRecipeDesc, new: &CraftingRecipeDesc| {
            temp_crafting_recipe_desc_tx_tx
                .send(SpacetimeUpdateMessages::Update {
                    old: old.clone(),
                    new: new.clone(),
                })
                .unwrap()
        },
    );
    let temp_crafting_recipe_desc_tx_tx = crafting_recipe_desc_tx.clone();
    ctx.db
        .crafting_recipe_desc()
        .on_insert(move |_ctx: &EventContext, new: &CraftingRecipeDesc| {
            temp_crafting_recipe_desc_tx_tx
                .send(SpacetimeUpdateMessages::Insert { new: new.clone() })
                .unwrap()
        });
    let temp_crafting_recipe_desc_tx_tx = crafting_recipe_desc_tx.clone();
    ctx.db
        .crafting_recipe_desc()
        .on_delete(move |_ctx: &EventContext, new: &CraftingRecipeDesc| {
            temp_crafting_recipe_desc_tx_tx
                .send(SpacetimeUpdateMessages::Remove {
                    delete: new.clone(),
                })
                .unwrap()
        });
    let tables_to_subscribe = vec![
        // "user_state",
        "mobile_entity_state",
        // "claim_tile_state",
        // "combat_action_desc",
        "item_desc",
        "cargo_desc",
        // "player_action_state",
        "crafting_recipe_desc",
        // "action_state",
        "player_state",
        "skill_desc",
        "player_username_state",
        // "building_desc",
        // "building_state",
        "vault_state",
        "experience_state",
        "claim_tech_state",
        "claim_state",
        "claim_member_state",
        "claim_local_state",
        "deployable_state",
        // "collectible_desc",
        "claim_tech_desc",
        // "claim_description_state", -> claim_state
        // "location_state",
        "inventory_state",
    ];

    ctx.subscription_builder()
        .on_applied(move |_ctx: &SubscriptionEventContext| {})
        .on_error(on_sub_error)
        .subscribe(
            tables_to_subscribe
                .into_iter()
                .filter_map(|table| {
                    if *remove_desc && table.contains("_desc") {
                        return None;
                    }

                    Some(format!("select * from {table}"))
                })
                .collect::<Vec<_>>(),
        );

    tokio::spawn(async move {
        let _ = ctx.run_async().await;
    });
}

pub fn start_websocket_bitcraft_logic(config: Config, global_app_state: Arc<AppState>) {
    start_websocket_bitcraft_logic_old(config.clone(), global_app_state.clone());
    tokio::spawn(async move {
        let (mobile_entity_state_tx, mobile_entity_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (player_state_tx, player_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (player_username_state_tx, player_username_state_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (experience_state_tx, experience_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (inventory_state_tx, inventory_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (item_desc_tx, item_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (cargo_desc_tx, cargo_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (vault_state_collectibles_tx, vault_state_collectibles_rx) =
            tokio::sync::mpsc::unbounded_channel();

        let (deployable_state_tx, deployable_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_state_tx, claim_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_local_state_tx, claim_local_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_member_state_tx, claim_member_state_rx) = tokio::sync::mpsc::unbounded_channel();

        let (skill_desc_tx, skill_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (claim_tech_state_tx, claim_tech_state_rx) = tokio::sync::mpsc::unbounded_channel();
        let (claim_tech_desc_tx, claim_tech_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let (crafting_recipe_desc_tx, crafting_recipe_desc_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut remove_desc = false;

        config.spacetimedb.databases.iter().for_each(|database| {
            connect_to_db_logic(
                &config,
                database,
                &remove_desc,
                &mobile_entity_state_tx,
                &player_state_tx,
                &player_username_state_tx,
                &experience_state_tx,
                &inventory_state_tx,
                &item_desc_tx,
                &cargo_desc_tx,
                &vault_state_collectibles_tx,
                &deployable_state_tx,
                &claim_state_tx,
                &claim_local_state_tx,
                &claim_member_state_tx,
                &skill_desc_tx,
                &claim_tech_state_tx,
                &claim_tech_desc_tx,
                &crafting_recipe_desc_tx,
            );

            remove_desc = true;
        });
        start_worker_mobile_entity_state(global_app_state.clone(), mobile_entity_state_rx);
        start_worker_player_state(
            global_app_state.clone(),
            player_state_rx,
            1000,
            Duration::from_millis(25),
        );
        start_worker_player_username_state(
            global_app_state.clone(),
            player_username_state_rx,
            1000,
            Duration::from_millis(25),
        );
        start_worker_experience_state(
            global_app_state.clone(),
            experience_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_inventory_state(
            global_app_state.clone(),
            inventory_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_vault_state_collectibles(
            global_app_state.clone(),
            vault_state_collectibles_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_item_desc(
            global_app_state.clone(),
            item_desc_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_cargo_desc(
            global_app_state.clone(),
            cargo_desc_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_deployable_state(
            global_app_state.clone(),
            deployable_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_claim_state(
            global_app_state.clone(),
            claim_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_claim_local_state(
            global_app_state.clone(),
            claim_local_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_claim_member_state(
            global_app_state.clone(),
            claim_member_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_skill_desc(
            global_app_state.clone(),
            skill_desc_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_claim_tech_state(
            global_app_state.clone(),
            claim_tech_state_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_claim_tech_desc(
            global_app_state.clone(),
            claim_tech_desc_rx,
            2000,
            Duration::from_millis(25),
        );
        start_worker_crafting_recipe_desc(
            global_app_state.clone(),
            crafting_recipe_desc_rx,
            2000,
            Duration::from_millis(25),
        );
    });
}

#[allow(dead_code)]
async fn websocket_retry_helper(
    reconnect_wait_time: u32,
    retry_count: &mut u32,
    max_retry_count: u32,
    backoff_factor: u32,
    now: Instant,
    was_connected: bool,
) -> bool {
    if now.elapsed() > Duration::from_secs(5) && was_connected {
        *retry_count = 1;
    }

    let wait_time = reconnect_wait_time * retry_count.pow(backoff_factor);

    tracing::debug!("Wait time {wait_time}");

    tokio::time::sleep(Duration::from_secs(wait_time as u64)).await;
    *retry_count += 1;
    if *retry_count > max_retry_count {
        return true;
    }

    tracing::info!("Reconnecting to websocket {retry_count} {max_retry_count}");
    false
}

enum SpacetimeUpdateMessages<T> {
    Insert { new: T },
    Update { old: T, new: T },
    Remove { delete: T },
}

fn start_worker_mobile_entity_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<MobileEntityState>>,
) {
    tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            match update {
                SpacetimeUpdateMessages::Insert { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                SpacetimeUpdateMessages::Update { new, .. } => {
                    let model: mobile_entity_state::Model = new.into();

                    global_app_state
                        .mobile_entity_state
                        .insert(model.entity_id, model.clone());

                    global_app_state
                        .tx
                        .send(WebSocketMessages::MobileEntityState(model))
                        .unwrap();
                }
                SpacetimeUpdateMessages::Remove { delete, .. } => {
                    global_app_state
                        .mobile_entity_state
                        .remove(&delete.entity_id);
                }
            }
        }
    });
}

fn start_worker_item_desc(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ItemDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(item_desc::Column::Id)
            .update_columns([
                item_desc::Column::Name,
                item_desc::Column::Description,
                item_desc::Column::Volume,
                item_desc::Column::Durability,
                item_desc::Column::ConvertToOnDurabilityZero,
                item_desc::Column::SecondaryKnowledgeId,
                item_desc::Column::ModelAssetName,
                item_desc::Column::IconAssetName,
                item_desc::Column::Tier,
                item_desc::Column::Tag,
                item_desc::Column::Rarity,
                item_desc::Column::CompendiumEntry,
                item_desc::Column::ItemListId,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::item_desc::Model = new.into();
                                global_app_state.item_desc.insert(model.id, model.clone());
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::item_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.item_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ItemDesc = id, error = error.to_string(), "Could not delete ItemDesc");
                                }

                                tracing::debug!("ItemDesc::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::item_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_cargo_desc(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<CargoDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(cargo_desc::Column::Id)
            .update_columns([
                cargo_desc::Column::Name,
                cargo_desc::Column::Description,
                cargo_desc::Column::Volume,
                cargo_desc::Column::SecondaryKnowledgeId,
                cargo_desc::Column::ModelAssetName,
                cargo_desc::Column::IconAssetName,
                cargo_desc::Column::CarriedModelAssetName,
                cargo_desc::Column::PickUpAnimationStart,
                cargo_desc::Column::PickUpAnimationEnd,
                cargo_desc::Column::DropAnimationStart,
                cargo_desc::Column::DropAnimationEnd,
                cargo_desc::Column::PickUpTime,
                cargo_desc::Column::PlaceTime,
                cargo_desc::Column::AnimatorState,
                cargo_desc::Column::MovementModifier,
                cargo_desc::Column::BlocksPath,
                cargo_desc::Column::OnDestroyYieldCargos,
                cargo_desc::Column::DespawnTime,
                cargo_desc::Column::Tier,
                cargo_desc::Column::Tag,
                cargo_desc::Column::Rarity,
                cargo_desc::Column::NotPickupable,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::cargo_desc::Model = new.into();
                                global_app_state.cargo_desc.insert(model.id, model.clone());
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::cargo_desc::Model = delete.into();
                                let id = model.id;
                                global_app_state.cargo_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CargoDesc = id, error = error.to_string(), "Could not delete CargoDesc");
                                }

                                tracing::debug!("CargoDesc::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::cargo_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_player_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::player_state::Column::EntityId)
            .update_columns([
                ::entity::player_state::Column::TimePlayed,
                ::entity::player_state::Column::SessionStartTimestamp,
                ::entity::player_state::Column::TimeSignedIn,
                ::entity::player_state::Column::SignInTimestamp,
                ::entity::player_state::Column::SignedIn,
                ::entity::player_state::Column::TeleportLocation,
                ::entity::player_state::Column::TravelerTasksExpiration,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerState = id, error = error.to_string(), "Could not delete PlayerState");
                                }

                                tracing::debug!("PlayerState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::player_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_player_username_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict =
            sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
                .update_columns([::entity::player_username_state::Column::Username])
                .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::player_username_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::player_username_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(PlayerUsernameState = id, error = error.to_string(), "Could not delete PlayerUsernameState");
                                }

                                tracing::debug!("PlayerUsernameState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::player_username_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_experience_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([::entity::experience_state::Column::Experience])
        .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let id = new.entity_id;
                                new.experience_stacks.iter().for_each(|es| {
                                    messages.push(::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    })
                                });

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, old } => {
                                let id = new.entity_id;

                                let mut new_level_vec = vec![];

                                new.experience_stacks.iter().for_each(|es| {
                                    new_level_vec.push((
                                        es.clone(),
                                        experience_to_level(es.quantity as i64),
                                    ));

                                    messages.push(::entity::experience_state::Model {
                                        entity_id: id as i64,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    })
                                });
                                old.experience_stacks.iter().for_each(|es| {
                                    let old_level =
                                        experience_to_level(es.quantity as i64);

                                    let new_level = new_level_vec.iter().find(|new_level| new_level.0.skill_id.eq(&es.skill_id));
                                    let skill_name = global_app_state.skill_desc.get(&(es.skill_id as i64));

                                    if let Some(skill_name) = skill_name {
                                        if let Some(new_level) = new_level {
                                            if old_level != new_level.1 {

                                                    global_app_state.tx.send(WebSocketMessages::Level {
                                                        level: new_level.1 as u64,
                                                        skill_name: skill_name.to_owned().name,
                                                        user_id: id as i64,
                                                    })
                                                    .expect("TODO: panic message");
                                                }

                                            if new_level.0.quantity > es.quantity {
                                                global_app_state.tx.send(WebSocketMessages::Experience {
                                                    level: new_level.1 as u64,
                                                    experience: new_level.0.quantity as u64,
                                                    rank: 0,
                                                    skill_name: skill_name.to_owned().name,
                                                    user_id: id as i64,
                                                })
                                                .expect("TODO: panic message");
                                            }
                                        }
                                    }
                                });




                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let id = delete.entity_id as i64;
                                let vec_es = delete.experience_stacks.iter().map(|es| {
                                    if let Some(index) = messages.iter().position(|value| value.skill_id == es.skill_id && value.entity_id == id) {
                                        messages.remove(index);
                                    }

                                    ::entity::experience_state::Model {
                                        entity_id: id,
                                        skill_id: es.skill_id,
                                        experience: es.quantity,
                                    }
                                }).collect::<Vec<_>>();

                                for es in vec_es {
                                    if let Err(error) = es.delete(&global_app_state.conn).await {
                                        tracing::error!(ExperienceState = id, error = error.to_string(), "Could not delete ExperienceState");
                                    }
                                }
                                tracing::debug!("ExperienceState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::experience_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_inventory_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<InventoryState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(::entity::inventory::Column::EntityId)
            .update_columns([
                ::entity::inventory::Column::Pockets,
                ::entity::inventory::Column::InventoryIndex,
                ::entity::inventory::Column::CargoIndex,
                ::entity::inventory::Column::OwnerEntityId,
                ::entity::inventory::Column::PlayerOwnerEntityId,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::inventory::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::inventory::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::inventory::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(InventoryState = id, error = error.to_string(), "Could not delete InventoryState");
                                }

                                tracing::debug!("InventoryState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::inventory::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_deployable_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<DeployableState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(deployable_state::Column::EntityId)
            .update_columns([
                deployable_state::Column::OwnerId,
                deployable_state::Column::ClaimEntityId,
                deployable_state::Column::Direction,
                deployable_state::Column::DeployableDescriptionId,
                deployable_state::Column::Nickname,
                deployable_state::Column::Hidden,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::deployable_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::deployable_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::deployable_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(DeployableState = id, error = error.to_string(), "Could not delete DeployableState");
                                }

                                tracing::debug!("DeployableState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::deployable_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_state::Column::EntityId)
            .update_columns([
                claim_state::Column::OwnerPlayerEntityId,
                claim_state::Column::OwnerBuildingEntityId,
                claim_state::Column::Name,
                claim_state::Column::Neutral,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::claim_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimState = id, error = error.to_string(), "Could not delete ClaimState");
                                }

                                tracing::debug!("ClaimState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_local_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimLocalState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_local_state::Column::EntityId)
            .update_columns([
                claim_local_state::Column::Supplies,
                claim_local_state::Column::BuildingMaintenance,
                claim_local_state::Column::NumTiles,
                claim_local_state::Column::NumTileNeighbors,
                claim_local_state::Column::Location,
                claim_local_state::Column::Treasury,
                claim_local_state::Column::XpGainedSinceLastCoinMinting,
                claim_local_state::Column::SuppliesPurchaseThreshold,
                claim_local_state::Column::SuppliesPurchasePrice,
                claim_local_state::Column::BuildingDescriptionId,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::claim_local_state::Model = new.into();

                                messages.push(model.clone());

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_local_state::Model = new.into();
                                messages.push(model.clone());

                                global_app_state.tx
                                    .send(WebSocketMessages::ClaimLocalState(
                                        model,
                                    ))
                                    .unwrap();

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_local_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }
                                global_app_state.claim_local_state.remove(&(model.entity_id as u64));

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimLocalState = id, error = error.to_string(), "Could not delete ClaimLocalState");
                                }

                                tracing::debug!("ClaimLocalState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_local_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| {
                            global_app_state
                                .claim_local_state
                                .insert(value.entity_id as u64, value.clone());
                            value.clone().into_active_model()
                        })
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_member_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimMemberState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(claim_member_state::Column::EntityId)
            .update_columns([
                claim_member_state::Column::ClaimEntityId,
                claim_member_state::Column::PlayerEntityId,
                claim_member_state::Column::UserName,
                claim_member_state::Column::InventoryPermission,
                claim_member_state::Column::BuildPermission,
                claim_member_state::Column::OfficerPermission,
                claim_member_state::Column::CoOwnerPermission,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::claim_member_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_member_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_member_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }


                                global_app_state.remove_claim_member(model.clone());

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimMemberState = id, error = error.to_string(), "Could not delete ClaimMemberState");
                                }

                                tracing::debug!("ClaimMemberState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!("Processing {} messages in batch", messages.len());
                let _ = ::entity::claim_member_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| {
                            global_app_state.add_claim_member(value.clone());
                            value.clone().into_active_model()
                        })
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_skill_desc(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<SkillDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(skill_desc::Column::Id)
            .update_columns([
                skill_desc::Column::Name,
                skill_desc::Column::Description,
                skill_desc::Column::IconAssetName,
                skill_desc::Column::Title,
                skill_desc::Column::SkillCategory,
                skill_desc::Column::Skill,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::skill_desc::Model = new.into();

                                global_app_state.skill_desc.insert(model.id, model.clone());
                                messages.push(model);

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::skill_desc::Model = new.into();
                                global_app_state.skill_desc.insert(model.id, model.clone());
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::skill_desc::Model = delete.into();
                                let id = model.id;

                                global_app_state.skill_desc.remove(&id);
                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(SkillDesc = id, error = error.to_string(), "Could not delete SkillDesc");
                                }

                                tracing::debug!("SkillDesc::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::skill_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_vault_state_collectibles(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<VaultState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([
            vault_state_collectibles::Column::EntityId,
            vault_state_collectibles::Column::Id,
        ])
        .update_columns([
            vault_state_collectibles::Column::Activated,
            vault_state_collectibles::Column::Count,
        ])
        .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                               let models = raw_model.to_model_collectibles();
                                for model in models {
                                    messages.push(model);
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = new.into();
                                let models = raw_model.to_model_collectibles();
                                for model in models {
                                    messages.push(model);
                                }
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let raw_model: ::entity::vault_state_collectibles::RawVaultState = delete.into();
                                let models= raw_model.to_model_collectibles();
                                for model in models {

                                    let id = model.entity_id;

                                    if let Some(index) = messages.iter().position(|value| value == &model) {
                                        messages.remove(index);
                                    }

                                    if let Err(error) = model.delete(&global_app_state.conn).await {
                                        tracing::error!(VaultState = id, error = error.to_string(), "Could not delete VaultState");
                                    }
                                }

                                tracing::debug!("VaultState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::vault_state_collectibles::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_tech_state(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechState>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([claim_tech_state::Column::EntityId])
            .update_columns([
                claim_tech_state::Column::Learned,
                claim_tech_state::Column::Researching,
                claim_tech_state::Column::StartTimestamp,
                claim_tech_state::Column::ScheduledId,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_tech_state::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_state::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_state::Model = delete.into();
                                let id = model.entity_id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechState = id, error = error.to_string(), "Could not delete ClaimTechState");
                                }

                                tracing::debug!("ClaimTechState::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!(
                    "ClaimTechState ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_state::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechState: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_claim_tech_desc(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::columns([::entity::claim_tech_desc::Column::Id])
            .update_columns([
                ::entity::claim_tech_desc::Column::Description,
                ::entity::claim_tech_desc::Column::Tier,
                ::entity::claim_tech_desc::Column::SuppliesCost,
                ::entity::claim_tech_desc::Column::ResearchTime,
                ::entity::claim_tech_desc::Column::Requirements,
                ::entity::claim_tech_desc::Column::Input,
                ::entity::claim_tech_desc::Column::Members,
                ::entity::claim_tech_desc::Column::Area,
                ::entity::claim_tech_desc::Column::Supplies,
                ::entity::claim_tech_desc::Column::XpToMintHexCoin,
            ])
            .to_owned();

        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();

                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::claim_tech_desc::Model = new.into();
                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::claim_tech_desc::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(ClaimTechDesc = id, error = error.to_string(), "Could not delete ClaimTechDesc");
                                }

                                tracing::debug!("ClaimTechDesc::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                tracing::debug!(
                    "ClaimTechDesc ->>>> Processing {} messages in batch",
                    messages.len()
                );
                let insert = ::entity::claim_tech_desc::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;

                if insert.is_err() {
                    tracing::error!("Error inserting ClaimTechDesc: {}", insert.unwrap_err())
                }
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

fn start_worker_crafting_recipe_desc(
    global_app_state: Arc<AppState>,
    mut rx: UnboundedReceiver<SpacetimeUpdateMessages<CraftingRecipeDesc>>,
    batch_size: usize,
    time_limit: Duration,
) {
    tokio::spawn(async move {
        let on_conflict = sea_query::OnConflict::column(crafting_recipe::Column::Id)
        .update_columns([
            crafting_recipe::Column::Name,
            crafting_recipe::Column::TimeRequirement,
            crafting_recipe::Column::StaminaRequirement,
            crafting_recipe::Column::ToolDurabilityLost,
            crafting_recipe::Column::BuildingRequirement,
            crafting_recipe::Column::LevelRequirements,
            crafting_recipe::Column::ToolRequirements,
            crafting_recipe::Column::ConsumedItemStacks,
            crafting_recipe::Column::DiscoveryTriggers,
            crafting_recipe::Column::RequiredKnowledges,
            crafting_recipe::Column::RequiredClaimTechId,
            crafting_recipe::Column::FullDiscoveryScore,
            crafting_recipe::Column::ExperiencePerProgress,
            crafting_recipe::Column::AllowUseHands,
            crafting_recipe::Column::CraftedItemStacks,
            crafting_recipe::Column::IsPassive,
            crafting_recipe::Column::ActionsRequired,
            crafting_recipe::Column::ToolMeshIndex,
            crafting_recipe::Column::RecipePerformanceId,
        ])
        .to_owned();


        loop {
            let mut messages = Vec::new();
            let timer = sleep(time_limit);
            tokio::pin!(timer);

            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        match msg {
                            SpacetimeUpdateMessages::Insert { new, .. } => {

                                let model: ::entity::crafting_recipe::Model = new.into();
                                messages.push(model);

                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Update { new, .. } => {
                                let model: ::entity::crafting_recipe::Model = new.into();                                messages.push(model);
                                if messages.len() >= batch_size {
                                    break;
                                }
                            }
                            SpacetimeUpdateMessages::Remove { delete,.. } => {
                                let model: ::entity::crafting_recipe::Model = delete.into();
                                let id = model.id;

                                if let Some(index) = messages.iter().position(|value| value == &model) {
                                    messages.remove(index);
                                }

                                if let Err(error) = model.delete(&global_app_state.conn).await {
                                    tracing::error!(CraftingRecipeDesc = id, error = error.to_string(), "Could not delete SkillDesc");
                                }

                                tracing::debug!("CraftingRecipeDesc::Remove");
                            }
                        }
                    }
                    _ = &mut timer => {
                        // Time limit reached
                        break;
                    }
                    else => {
                        // Channel closed and no more messages
                        break;
                    }
                }
            }

            if !messages.is_empty() {
                //tracing::info!("Processing {} messages in batch", messages.len());
                let _ = ::entity::crafting_recipe::Entity::insert_many(
                    messages
                        .iter()
                        .map(|value| value.clone().into_active_model())
                        .collect::<Vec<_>>(),
                )
                .on_conflict(on_conflict.clone())
                .exec(&global_app_state.conn)
                .await;
                // Your batch processing logic here
            }

            // If the channel is closed and we processed the last batch, exit the outer loop
            if messages.is_empty() && rx.is_closed() {
                break;
            }
        }
    });
}

//
// async fn create_websocket_connection(config: &Config) -> anyhow::Result<WebSocket> {
//     let mut headers = HeaderMap::new();
//     headers.insert(
//         "Authorization",
//         format!("Bearer {}", config.spacetimedb.password).parse()?,
//     );
//     headers.insert(SEC_WEBSOCKET_PROTOCOL, "v1.json.spacetimedb".parse()?);
//     headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==".parse()?);
//     headers.insert("Sec-WebSocket-Version", "13".parse()?);
//     headers.insert(
//         reqwest::header::USER_AGENT,
//         format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION")).parse()?,
//     );
//
//     let response = ClientBuilder::default()
//         .timeout(Duration::from_millis(5000))
//         .connect_timeout(Duration::from_millis(2500))
//         .build()?
//         .get(format!(
//             "{}/{}/{}/{}",
//             config.weboosocket_url(),
//             "database",
//             config.spacetimedb.database,
//             "subscribe"
//         ))
//         .headers(headers)
//         .upgrade()
//         // .web_socket_config(tungstenite::protocol::WebSocketConfig::default()
//         //    .max_frame_size(Some(1024 * 1024 * 1500))
//         //    .max_message_size(Some(1024 * 1024 * 1500))
//         // )
//         .web_socket_config(tungstenite::protocol::WebSocketConfig {
//             max_frame_size: Some(1024 * 1024 * 1500),
//             max_message_size: Some(1024 * 1024 * 1500),
//             ..Default::default()
//         })
//         .protocols(vec!["v1.json.spacetimedb"])
//         .send()
//         .await?;
//
//     Ok(response.into_websocket().await?)
// }

//
// fn start_websocket_message_thread(
//     broadcast_tx: UnboundedSender<WebSocketMessages>,
//     global_app_state: Arc<AppState>,
//     mut rx: UnboundedReceiver<WebSocketMessage>,
//     tmp_config: Config,
// ) {
//     tokio::spawn(async move {
//         let db = crate::create_importer_default_db_connection(tmp_config.clone()).await;
//
//         let skill_id_to_skill_name = skill_desc::Entity::find()
//             .select_only()
//             .column(skill_desc::Column::Id)
//             .column(skill_desc::Column::Name)
//             .into_tuple::<(i64, String)>()
//             .all(&db)
//             .await
//             .unwrap()
//             .into_iter()
//             .collect::<HashMap<i64, String>>();
//
//         loop {
//             let mut evenets = Vec::with_capacity(1000);
//             let mut tables: HashMap<String, Vec<TableWithOriginalEventTransactionUpdate>> =
//                 HashMap::new();
//             let db = db.clone();
//
//             let count = rx.recv_many(&mut evenets, 1000).await;
//             let mut raw_events_data = vec![];
//
//             for event in evenets.iter() {
//                 if let WebSocketMessage::TransactionUpdate(transaction_update) = event {
//                     let mut compressor = async_compression::tokio::write::ZstdEncoder::new(vec![]);
//                     let _ = compressor
//                         .write_all(
//                             serde_json::to_string(&transaction_update)
//                                 .unwrap()
//                                 .as_bytes(),
//                         )
//                         .await;
//                     compressor.flush().await.unwrap();
//                     compressor.shutdown().await.unwrap();
//
//                     let user_id = transaction_update.caller_identity.__identity__.clone();
//
//                     let user_id = if let Some(user_id_ref) = global_app_state.connected_user_map.get(&user_id.to_string()) {
//                         Some(user_id_ref.to_owned())
//                     } else {
//                         None
//                     };
//
//                     raw_events_data.push(
//                         RawEventData {
//                             timestamp: transaction_update
//                                 .timestamp
//                                 .__timestamp_micros_since_unix_epoch__,
//                             request_id: transaction_update.reducer_call.request_id as i64,
//                             reducer_name: transaction_update
//                                 .reducer_call
//                                 .reducer_name
//                                 .clone()
//                                 .parse()
//                                 .unwrap(),
//                             reducer_id: transaction_update.reducer_call.reducer_id as i64,
//                             event_data: compressor.into_inner(),
//                             user_id,
//                         }
//                             .into_active_model(),
//                     );
//                 }
//             }
//
//             //raw_event_data::Entity::insert_many(raw_events_data)
//             //    .exec(&db)
//             //    .await
//             //    .unwrap();
//
//             for event in evenets.iter() {
//                 match event {
//                     WebSocketMessage::TransactionUpdate(transaction_update) => {
//                         metrics::counter!(
//                             "websocket.message.count",
//                             &[("type", "TransactionUpdate"),]
//                         )
//                             .increment(1);
//
//                         if transaction_update.status.failed.is_some() {
//                             error!(
//                                 "Transaction with error {}",
//                                 transaction_update.status.failed.as_ref().unwrap()
//                             );
//                             continue;
//                         }
//
//                         if transaction_update
//                             .status
//                             .committed
//                             .as_ref()
//                             .unwrap()
//                             .tables
//                             .is_empty()
//                         {
//                             continue;
//                         }
//
//                         for table in transaction_update
//                             .status
//                             .committed
//                             .as_ref()
//                             .unwrap()
//                             .tables
//                             .iter()
//                         {
//                             metrics::counter!(
//                                 "websocket_message_table_count",
//                                 &[
//                                     ("type", "TransactionUpdate".to_string()),
//                                     ("table", format!("{}", table.table_name)),
//                                 ]
//                             )
//                                 .increment(1);
//
//                             if let Some(table_vec) = tables.get_mut(&table.table_name.to_string()) {
//                                 //TODO this probebly has to be rewriten
//                                 table.updates.iter().for_each(|updates| {
//                                     table_vec.push(TableWithOriginalEventTransactionUpdate {
//                                         table_id: table.table_id,
//                                         table_name: table.table_name.clone(),
//                                         deletes: updates.deletes.clone(),
//                                         inserts: updates.inserts.clone(),
//                                         original_event: transaction_update.clone(),
//                                     });
//                                 })
//                             } else {
//                                 tables.insert(
//                                     table.table_name.clone().as_ref().to_string(),
//                                     table
//                                         .updates
//                                         .iter()
//                                         .map(|updates| TableWithOriginalEventTransactionUpdate {
//                                             table_id: table.table_id,
//                                             table_name: table.table_name.clone(),
//                                             deletes: updates.deletes.clone(),
//                                             inserts: updates.inserts.clone(),
//                                             original_event: transaction_update.clone(),
//                                         })
//                                         .collect::<Vec<TableWithOriginalEventTransactionUpdate>>(),
//                                 );
//                             }
//                         }
//
//                         metrics::gauge!(
//                             "websocket_message_inflight_gauge",
//                             &[("type", "TransactionUpdate"),]
//                         )
//                             .decrement(1);
//                     }
//                     WebSocketMessage::InitialSubscription(subscription_update) => {
//                         metrics::counter!(
//                             "websocket.message.count",
//                             &[("type", "InitialSubscription"),]
//                         )
//                             .increment(1);
//
//                         if subscription_update.database_update.tables.is_empty() {
//                             continue;
//                         }
//
//                         for table in subscription_update.database_update.tables.iter() {
//                             metrics::counter!(
//                                 "websocket_message_table_count",
//                                 &[
//                                     ("type", "InitialSubscription".to_string()),
//                                     ("table", format!("{}", table.table_name)),
//                                 ]
//                             )
//                                 .increment(1);
//
//                             let start = std::time::Instant::now();
//
//                             if table.table_name.as_ref() == "user_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let user_state: user_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(user_state) => user_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert user_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.connected_user_map.insert(
//                                             user_state.identity.__identity__,
//                                             user_state.entity_id,
//                                         );
//                                     }
//                                 }
//                             }
//                             if table.table_name.as_ref() == "player_username_state" {
//                                 let result =
//                                     player_state::handle_initial_subscription_player_username_state(&db, table)
//                                         .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "player_username_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "player_state" {
//                                 let result =
//                                     player_state::handle_initial_subscription_player_state(
//                                         &db, table,
//                                     )
//                                         .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "player_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "experience_state" {
//                                 info!("experience_state initial subscription");
//                                 let result =
//                                     leaderboard::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "experience_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "building_state" {
//                                 let result =
//                                     buildings::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "building_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "building_desc" {
//                                 let result = buildings::handle_initial_subscription_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "building_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//                             if table.table_name.as_ref() == "inventory_state" {
//                                 let result =
//                                     inventory::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "inventory_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "item_desc" {
//                                 let result = items::handle_initial_subscription_item_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "item_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "cargo_desc" {
//                                 let result = cargo_desc::handle_initial_subscription_cargo_desc(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "cargo_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tech_state" {
//                                 let result = claim_tech_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_tech_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "skill_desc" {
//                                 let result = skill_descriptions::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "skill_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tech_desc" {
//                                 let result = crate::claim_tech_desc::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_tech_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_state" {
//                                 let result = crate::claims::claim_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_local_state" {
//                                 let result = crate::claims::claim_local_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_local_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_local_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_member_state" {
//                                 let result = crate::claims::claim_member_state::handle_initial_subscription(
//                                     &global_app_state,
//                                     table,
//                                 )
//                                     .await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "claim_member_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 } else {
//                                     info!(
//                                         "claim_member_state initial subscription success",
//                                     );
//                                 }
//                             }
//
//                             // if table.table_name.as_ref() == "claim_description_state" {
//                             //     let result =
//                             //         claims::handle_initial_subscription(&global_app_state, table)
//                             //             .await;
//                             //
//                             //     if result.is_err() {
//                             //         error!(
//                             //             "claim_description_state initial subscription failed: {:?}",
//                             //             result.err()
//                             //         );
//                             //     }
//                             // }
//
//                             if table.table_name.as_ref() == "deployable_state" {
//                                 let result =
//                                     deployable_state::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "deployable_state initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "vault_state_collectibles" {
//                                 let result =
//                                     vault_state_collectibles::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "vault_state_collectibles initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "collectible_desc" {
//                                 let result =
//                                     collectible_desc::handle_initial_subscription(&db, table).await;
//
//                                 if result.is_err() {
//                                     error!(
//                                         "collectible_desc initial subscription failed: {:?}",
//                                         result.err()
//                                     );
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "mobile_entity_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let mobile_entity_state: entity::mobile_entity_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(mobile_entity_state) => mobile_entity_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.mobile_entity_state.insert(
//                                             mobile_entity_state.entity_id,
//                                             mobile_entity_state.clone(),
//                                         );
//
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MobileEntityState(
//                                                 mobile_entity_state,
//                                             ))
//                                             .unwrap();
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "claim_tile_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let claim_tile_state: entity::claim_tile_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(claim_tile_state) => claim_tile_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.claim_tile_state.insert(
//                                             claim_tile_state.entity_id,
//                                             claim_tile_state.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "crafting_recipe_desc" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let crafting_recipe_desc: entity::crafting_recipe::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(crafting_recipe_desc) => crafting_recipe_desc,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert crafting_recipe_desc Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.crafting_recipe_desc.insert(
//                                             crafting_recipe_desc.id,
//                                             crafting_recipe_desc.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "player_action_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let player_action_state: entity::player_action_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(player_action_state) => player_action_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert player_action_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.player_action_state.insert(
//                                             player_action_state.entity_id,
//                                             player_action_state.clone(),
//                                         );
//                                     }
//                                 }
//                             }
//
//                             if table.table_name.as_ref() == "location_state" {
//                                 let mut num_entries = 0;
//                                 for update in table.updates.iter() {
//                                     num_entries = update.inserts.len();
//                                     for row in update.inserts.iter() {
//                                         let location_state: entity::location::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(location_state) => location_state,
//                                                 Err(error) => {
//                                                     error!("InitialSubscription Insert location_state Error: {:?} -> {:?}", error, row);
//                                                     continue;
//                                                 }
//                                             };
//
//                                         global_app_state.location_state.insert(
//                                             location_state.entity_id,
//                                             location_state.clone(),
//                                         );
//                                     }
//                                 }
//
//                                 info!(
//                                     "location_state initial subscription success",
//                                 );
//                             }
//
//                             if table.table_name.as_ref() == "action_state" {
//                                 for update in table.updates.iter() {
//                                     for row in update.inserts.iter() {
//                                         let action_state: entity::action_state::Model =
//                                             match serde_json::from_str(row) {
//                                                 Ok(action_state) => action_state,
//                                                 Err(error) => {
//                                                     error!(
//                                                         "InitialSubscription Insert action_state Error: {:?} -> {:?}",
//                                                         error, row
//                                                     );
//                                                     continue;
//                                                 }
//                                             };
//
//                                         if let Some(action_states) = global_app_state
//                                             .action_state
//                                             .get_mut(&action_state.owner_entity_id)
//                                         {
//                                             action_states.insert(
//                                                 action_state.entity_id,
//                                                 action_state.clone(),
//                                             );
//                                         } else {
//                                             let action_states = dashmap::DashMap::new();
//                                             action_states.insert(
//                                                 action_state.entity_id,
//                                                 action_state.clone(),
//                                             );
//                                             global_app_state.action_state.insert(
//                                                 action_state.owner_entity_id,
//                                                 action_states,
//                                             );
//                                         }
//                                     }
//                                 }
//                             }
//
//                             metrics::histogram!(
//                                 "bitraft_event_handler_initial_subscription_duration_seconds",
//                                 &[("table", table.table_name.as_ref().to_string())]
//                             )
//                                 .record(start.elapsed().as_secs_f64());
//                         }
//
//                         metrics::gauge!(
//                             "websocket_message_inflight_gauge",
//                             &[("type", "InitialSubscription"),]
//                         )
//                             .decrement(1);
//                     }
//                     WebSocketMessage::IdentityToken(identity_token) => {
//                         println!("IdentityToken: {identity_token:?}");
//                     }
//                 }
//             }
//
//             for (table_name, table) in tables.iter() {
//                 debug!("Received table: {table_name} -> {:?}", table.len());
//                 let start = std::time::Instant::now();
//
//                 if table_name == "user_state" {
//                     for row in table.iter() {
//                         if row.inserts.is_empty() {
//                             continue;
//                         }
//
//                         match serde_json::from_str::<user_state::Model>(&row.inserts[0]) {
//                             Ok(user_state) => {
//                                 global_app_state.connected_user_map
//                                     .insert(user_state.identity.__identity__, user_state.entity_id);
//                             }
//                             Err(error) => {
//                                 error!("InitialSubscription Insert UserState Error: {:?}", error);
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "player_username_state" {
//                     let result =
//                         player_state::handle_transaction_update_player_username_state(&db, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "player_username_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//                 if table_name == "player_state" {
//                     let result = player_state::handle_transaction_update_player_state(
//                         &db,
//                         table,
//                         broadcast_tx.clone(),
//                     )
//                         .await;
//
//                     if result.is_err() {
//                         error!("player_state transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "experience_state" {
//                     let result = leaderboard::handle_transaction_update(
//                         &db,
//                         table,
//                         &skill_id_to_skill_name,
//                         broadcast_tx.clone(),
//                     )
//                         .await;
//
//                     if result.is_err() {
//                         error!(
//                             "experience_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "inventory_state" {
//                     let result = inventory::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "inventory_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "building_state" {
//                     let result = buildings::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "building_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "building_desc" {
//                     // let result = buildings::handle_transaction_update_desc(&global_app_state, table).await;
//                     //
//                     // if result.is_err() {
//                     //     error!(
//                     //         "building_desc transaction update failed: {:?}",
//                     //         result.err()
//                     //     );
//                     // }
//                 }
//
//                 if table_name == "item_desc" {
//                     let result =
//                         items::handle_transaction_update_item_desc(&global_app_state, table).await;
//
//                     if result.is_err() {
//                         error!("item_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "cargo_desc" {
//                     let result =
//                         cargo_desc::handle_transaction_update_cargo_desc(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!("cargo_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "claim_tech_state" {
//                     let result =
//                         claim_tech_state::handle_transaction_update(&global_app_state, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_tech_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "skill_desc" {
//                     let result =
//                         skill_descriptions::handle_transaction_update(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!("skill_desc transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "claim_tech_desc" {
//                     let result =
//                         crate::claim_tech_desc::handle_transaction_update(&global_app_state, table)
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_tech_desc transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_state" {
//                     let result =
//                         crate::claims::claim_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_member_state" {
//                     let result =
//                         crate::claims::claim_member_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_member_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "claim_local_state" {
//                     let result =
//                         crate::claims::claim_local_state::handle_transaction_update(&global_app_state, table, broadcast_tx.clone())
//                             .await;
//
//                     if result.is_err() {
//                         error!(
//                             "claim_local_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 // if table_name == "claim_description_state" {
//                 //     let result = claims::handle_transaction_update(
//                 //         &global_app_state,
//                 //         table,
//                 //         broadcast_tx.clone(),
//                 //     )
//                 //         .await;
//                 //
//                 //     if result.is_err() {
//                 //         error!(
//                 //             "claim_description_state transaction update failed: {:?}",
//                 //             result.err()
//                 //         );
//                 //     }
//                 // }
//
//                 if table_name == "deployable_state" {
//                     let result = deployable_state::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!(
//                             "deployable_state transaction update failed: {:?}",
//                             result.err()
//                         );
//                     }
//                 }
//
//                 if table_name == "vault_state_collectibles" {
//                     let result = vault_state_collectibles::handle_transaction_update(&db, table).await;
//
//                     if result.is_err() {
//                         error!("vault_state_collectibles transaction update failed: {:?}", result.err());
//                     }
//                 }
//
//                 if table_name == "mobile_entity_state" {
//                     for current_table in table.iter() {
//                         let mut old_data = HashMap::new();
//
//                         for row in current_table.deletes.iter() {
//                             let mobile_entity_state: entity::mobile_entity_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(mobile_entity_state) => mobile_entity_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             old_data
//                                 .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());
//                         }
//
//                         for row in current_table.inserts.iter() {
//                             let mobile_entity_state: entity::mobile_entity_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(mobile_entity_state) => mobile_entity_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert mobile_entity_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .mobile_entity_state
//                                 .insert(mobile_entity_state.entity_id, mobile_entity_state.clone());
//
//                             if !global_app_state.connected_user_map.iter().any(|connected_user| {
//                                 *connected_user == mobile_entity_state.entity_id as i64
//                             }) {
//                                 continue;
//                             }
//
//                             if let Some(old_data) = old_data.get(&mobile_entity_state.entity_id) {
//                                 let new_location_x = if mobile_entity_state.location_x == 0 {
//                                     mobile_entity_state.location_x
//                                 } else {
//                                     mobile_entity_state.location_x / 3 / 1000
//                                 };
//
//                                 let new_location_z = if mobile_entity_state.location_z == 0 {
//                                     mobile_entity_state.location_z
//                                 } else {
//                                     mobile_entity_state.location_z / 3 / 1000
//                                 };
//
//                                 let old_location_x = if old_data.location_x == 0 {
//                                     old_data.location_x
//                                 } else {
//                                     old_data.location_x / 3 / 1000
//                                 };
//
//                                 let old_location_z = if old_data.location_z == 0 {
//                                     old_data.location_z
//                                 } else {
//                                     old_data.location_z / 3 / 1000
//                                 };
//
//                                 let change_x = new_location_x - old_location_x;
//                                 let change_z = new_location_z - old_location_z;
//
//                                 if change_x == 0 && change_z == 0 {
//                                     continue;
//                                 }
//
//                                 match (
//                                     global_app_state
//                                         .claim_tile_state
//                                         .get(&mobile_entity_state.chunk_index),
//                                     global_app_state.claim_tile_state.get(&old_data.chunk_index),
//                                 ) {
//                                     (Some(new_chunk), Some(old_chunk)) => {
//                                         let new_chunk = new_chunk.value();
//                                         let old_chunk = old_chunk.value();
//
//                                         if new_chunk.claim_id != old_chunk.claim_id {
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::MovedOutOfClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: old_data.chunk_index,
//                                                     claim_id: old_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::PlayerMovedOutOfClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: old_data.chunk_index,
//                                                     claim_id: old_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::MovedIntoClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: mobile_entity_state.chunk_index,
//                                                     claim_id: new_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//
//                                             broadcast_tx
//                                                 .send(WebSocketMessages::PlayerMovedIntoClaim {
//                                                     user_id: mobile_entity_state.entity_id as i64,
//                                                     chunk_index: mobile_entity_state.chunk_index,
//                                                     claim_id: new_chunk.claim_id,
//                                                 })
//                                                 .unwrap();
//                                         }
//                                     }
//                                     (Some(new_chunk), None) => {
//                                         let new_chunk = new_chunk.value();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MovedIntoClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: mobile_entity_state.chunk_index,
//                                                 claim_id: new_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::PlayerMovedIntoClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: mobile_entity_state.chunk_index,
//                                                 claim_id: new_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                     }
//                                     (_, Some(old_chunk)) => {
//                                         let old_chunk = old_chunk.value();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::MovedOutOfClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: old_data.chunk_index,
//                                                 claim_id: old_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                         broadcast_tx
//                                             .send(WebSocketMessages::PlayerMovedOutOfClaim {
//                                                 user_id: mobile_entity_state.entity_id as i64,
//                                                 chunk_index: old_data.chunk_index,
//                                                 claim_id: old_chunk.claim_id,
//                                             })
//                                             .unwrap();
//                                     }
//                                     (_, _) => {}
//                                 }
//
//                                 broadcast_tx
//                                     .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
//                                     .unwrap();
//                             } else {
//                                 broadcast_tx
//                                     .send(WebSocketMessages::MobileEntityState(mobile_entity_state))
//                                     .unwrap();
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "claim_tile_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let claim_tile_state: entity::claim_tile_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(claim_tile_state) => claim_tile_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert claim_tile_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .claim_tile_state
//                                 .insert(claim_tile_state.entity_id, claim_tile_state.clone());
//                         }
//                     }
//                 }
//
//                 if table_name == "action_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let action_state: entity::action_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(action_state) => action_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert action_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             broadcast_tx
//                                 .send(WebSocketMessages::ActionState(action_state.clone()))
//                                 .unwrap();
//                             if let Some(action_states) = global_app_state
//                                 .action_state
//                                 .get_mut(&action_state.owner_entity_id)
//                             {
//                                 action_states.insert(action_state.entity_id, action_state.clone());
//                             } else {
//                                 let action_states = dashmap::DashMap::new();
//                                 action_states.insert(action_state.entity_id, action_state.clone());
//                                 global_app_state
//                                     .action_state
//                                     .insert(action_state.owner_entity_id, action_states);
//                             }
//                         }
//                     }
//                 }
//
//                 if table_name == "player_action_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//
//                             let player_action_state: entity::player_action_state::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(player_action_state) => player_action_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert player_action_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             let old_player_action_state = global_app_state
//                                 .player_action_state
//                                 .get(&player_action_state.entity_id);
//                             if old_player_action_state.is_none() {
//                                 broadcast_tx
//                                     .send(WebSocketMessages::PlayerActionStateChangeName(
//                                         player_action_state.action_type.get_action_name(),
//                                         player_action_state.entity_id,
//                                     ))
//                                     .unwrap();
//                             } else {
//                                 let old_player_action_state = old_player_action_state.unwrap();
//                                 if old_player_action_state.action_type
//                                     != player_action_state.action_type
//                                 {
//                                     broadcast_tx
//                                         .send(WebSocketMessages::PlayerActionStateChangeName(
//                                             player_action_state.action_type.get_action_name(),
//                                             player_action_state.entity_id,
//                                         ))
//                                         .unwrap();
//                                 }
//                             }
//
//                             broadcast_tx
//                                 .send(WebSocketMessages::PlayerActionState(
//                                     player_action_state.clone(),
//                                 ))
//                                 .unwrap();
//
//                             global_app_state
//                                 .player_action_state
//                                 .insert(player_action_state.entity_id, player_action_state.clone());
//                         }
//                     }
//                 }
//
//                 if table_name == "location_state" {
//                     for current_table in table.iter() {
//                         for row in current_table.inserts.iter() {
//                             let location_state: entity::location::Model =
//                                 match serde_json::from_str(row) {
//                                     Ok(location_state) => location_state,
//                                     Err(error) => {
//                                         error!(
//                                             "InitialSubscription Insert location_state Error: {:?} -> {:?}",
//                                             error, row
//                                         );
//                                         continue;
//                                     }
//                                 };
//
//                             global_app_state
//                                 .location_state
//                                 .insert(location_state.entity_id, location_state.clone());
//                         }
//                     }
//                 }
//
//                 metrics::histogram!(
//                     "bitraft_event_handler_transaction_update_duration_seconds",
//                     &[("table", table_name.to_string())]
//                 )
//                     .record(start.elapsed().as_secs_f64());
//             }
//
//             debug!("Received {count} events");
//             evenets.clear();
//             tokio::time::sleep(Duration::from_millis(50)).await;
//         }
//     });
// }

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "t", content = "c")]
pub(crate) enum WebSocketMessages {
    Subscribe {
        topics: Vec<String>,
    },
    ListSubscribedTopics,
    SubscribedTopics(Vec<String>),
    Unsubscribe {
        topic: String,
    },
    MobileEntityState(entity::mobile_entity_state::Model),
    Experience {
        experience: u64,
        level: u64,
        rank: u64,
        skill_name: String,
        user_id: i64,
    },
    TotalExperience {
        user_id: i64,
        experience: u64,
        experience_per_hour: u64,
    },
    MovedOutOfClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    MovedIntoClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerMovedIntoClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerMovedOutOfClaim {
        user_id: i64,
        chunk_index: u64,
        claim_id: u64,
    },
    PlayerActionState(entity::player_action_state::Model),
    PlayerActionStateChangeName(String, u64),
    Level {
        level: u64,
        user_id: i64,
        skill_name: String,
    },
    PlayerState(entity::player_state::Model),
    // ClaimDescriptionState(entity::claim_description_state::Model),
    ClaimLocalState(entity::claim_local_state::Model),
    Message(String),
    ActionState(entity::action_state::Model),
}

impl WebSocketMessages {
    pub fn topics(&self) -> Option<Vec<(String, i64)>> {
        match self {
            WebSocketMessages::Experience {
                skill_name,
                user_id,
                ..
            } => Some(vec![
                (format!("experience:{}", skill_name), *user_id),
                ("experience".to_string(), *user_id),
            ]),
            WebSocketMessages::ClaimLocalState(claim_local_state) => Some(vec![(
                "claim_local_state".to_string(),
                claim_local_state.entity_id,
            )]),
            WebSocketMessages::Level {
                user_id,
                skill_name,
                ..
            } => Some(vec![
                (format!("level:{}", skill_name), *user_id),
                ("level".to_string(), *user_id),
            ]),
            WebSocketMessages::PlayerMovedIntoClaim { user_id, .. } => {
                Some(vec![("player_moved_into_claim".to_string(), *user_id)])
            }
            WebSocketMessages::PlayerMovedOutOfClaim { user_id, .. } => {
                Some(vec![("player_moved_out_of_claim".to_string(), *user_id)])
            }
            WebSocketMessages::MovedOutOfClaim { claim_id, .. } => {
                Some(vec![("moved_out_of_claim".to_string(), *claim_id as i64)])
            }
            WebSocketMessages::MovedIntoClaim { claim_id, .. } => {
                Some(vec![("moved_into_claim".to_string(), *claim_id as i64)])
            }
            WebSocketMessages::PlayerState(player) => {
                Some(vec![("player_state".to_string(), player.entity_id)])
            }
            WebSocketMessages::MobileEntityState(mobile_entity_state) => Some(vec![(
                "mobile_entity_state".to_string(),
                mobile_entity_state.entity_id as i64,
            )]),
            // WebSocketMessages::ClaimDescriptionState(claim) => {
            //     Some(vec![("claim".to_string(), claim.entity_id)])
            // }
            WebSocketMessages::TotalExperience { user_id, .. } => {
                Some(vec![("total_experience".to_string(), *user_id)])
            }
            WebSocketMessages::PlayerActionState(player_action_state) => Some(vec![(
                "player_action_state".to_string(),
                player_action_state.entity_id as i64,
            )]),
            WebSocketMessages::PlayerActionStateChangeName(_, id) => Some(vec![(
                "player_action_state_change_name".to_string(),
                *id as i64,
            )]),
            WebSocketMessages::ActionState(action_state) => Some(vec![(
                "action_state".to_string(),
                action_state.owner_entity_id as i64,
            )]),
            WebSocketMessages::ListSubscribedTopics => None,
            WebSocketMessages::Subscribe { .. } => None,
            WebSocketMessages::SubscribedTopics(_) => None,
            WebSocketMessages::Unsubscribe { .. } => None,
            WebSocketMessages::Message(_) => None,
        }
    }
}

async fn create_websocket_connection(config: &Config) -> anyhow::Result<WebSocket> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Authorization",
        format!("Bearer {}", config.spacetimedb.password).parse()?,
    );
    headers.insert(SEC_WEBSOCKET_PROTOCOL, "v1.json.spacetimedb".parse()?);
    headers.insert("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==".parse()?);
    headers.insert("Sec-WebSocket-Version", "13".parse()?);
    headers.insert(
        reqwest::header::USER_AGENT,
        format!("Bitcraft-Hub-Api/{}", env!("CARGO_PKG_VERSION")).parse()?,
    );

    let response = ClientBuilder::default()
        .timeout(Duration::from_millis(5000))
        .connect_timeout(Duration::from_millis(2500))
        .build()?
        .get(format!(
            "{}/{}/{}/{}",
            config.weboosocket_url(),
            "database",
            "spacetime-control",
            "subscribe"
        ))
        .headers(headers)
        .upgrade()
        // .web_socket_config(tungstenite::protocol::WebSocketConfig::default()
        //    .max_frame_size(Some(1024 * 1024 * 1500))
        //    .max_message_size(Some(1024 * 1024 * 1500))
        // )
        .web_socket_config(tungstenite::protocol::WebSocketConfig {
            max_frame_size: Some(1024 * 1024 * 1500),
            max_message_size: Some(1024 * 1024 * 1500),
            ..Default::default()
        })
        .protocols(vec!["v1.json.spacetimedb"])
        .send()
        .await?;
    Ok(response.into_websocket().await?)
}

pub fn start_websocket_bitcraft_logic_old(
    config: Config,
    global_app_state: Arc<AppState>,
) {
    tokio::spawn(async move {
        let reconnect_wait_time_sec = 5;
        let mut retry_count = 1_u32;
        let max_retry_count = 10;
        let backoff_factor = 2;

        let tables_to_subscribe = vec![
            "program",
        ];

        let select_querys = tables_to_subscribe
            .iter()
            .map(|table_name| format!("SELECT * FROM {};", table_name))
            .collect::<Vec<String>>();


        loop {
            let now = Instant::now();
            let websocket = create_websocket_connection(&config).await;

            if websocket.is_err()
                && websocket_retry_helper(
                reconnect_wait_time_sec,
                &mut retry_count,
                max_retry_count,
                backoff_factor,
                now,
                false,
            )
                .await
            {
                tracing::error!(
                    "Could not connect to bitcraft server with following error {websocket:?}"
                );
                break;
            } else if websocket.is_err() {
                tracing::error!(
                    "Could not connect to bitcraft server with following error {websocket:?}"
                );
                continue;
            }

            tracing::info!("Websocket connection established");

            let mut websocket = websocket.unwrap();

            websocket
                .send(Message::Text(
                    serde_json::json!({
                        "Subscribe": {
                            "query_strings": select_querys,
                            "request_id": 1,
                        },
                    })
                        .to_string(),
                ))
                .await
                .unwrap();

            tracing::info!("Websocket send Subscribe query");

            while let result = websocket.try_next().await {
                if result.is_err() {
                    let error = result.unwrap_err();
                    tracing::error!("WebSocket message could not be decoded {error:?}");
                    break;
                }

                if let Ok(Some(message)) = result {
                    if let Message::Text(text) = message {
                        let message: Result<WebSocketMessage, serde_json::Error> =
                            serde_json::from_str(&text);

                        if message.is_err() {
                            //info!("Text: {:?}", text);
                            error!("Error: {:?}, text: {text}", message.err());
                            continue;
                        }

                        let message = message.unwrap();

                        match &message {
                            WebSocketMessage::TransactionUpdate(transaction_update) => {
                            
                            }
                            WebSocketMessage::InitialSubscription(subscription_update) => {
                                let inserts = &subscription_update.database_update.tables[0].updates[0].inserts;
                                for insert in inserts {
                                    let value: Value = serde_json::from_str(insert).unwrap();
                                    if value.as_object().unwrap().get("hash").unwrap() == "9a2bb349f418def32dd52b4e51799c88c4ae10d924fb8970e2e2ef9fabfdc1f9" {
                                        let mut file = File::create("foo3.txt").unwrap();
                                        let mut text = value.as_object().unwrap().get("bytes").unwrap().as_str().unwrap().to_string();
                                        file.write_all(&hex::decode(text.to_string()).unwrap()).unwrap();
                                    }
                                }
                            }
                            WebSocketMessage::IdentityToken(identity_token) => {
                                debug!("Received identity token: {identity_token:?}");
                            }
                        }
                    } else if let Message::Ping(_) = message {
                    } else {
                        warn!("Message: {:?}", message);
                    }
                }
            }

            if websocket_retry_helper(
                reconnect_wait_time_sec,
                &mut retry_count,
                max_retry_count,
                backoff_factor,
                now,
                true,
            )
                .await
            {
                break;
            }
        }
    });
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct InternalTransactionUpdate {
    pub(crate) user: Option<i64>,
    pub(crate) tables: Vec<TempTable>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) enum WebSocketMessage {
    IdentityToken(IdentityToken),
    TransactionUpdate(TransactionUpdate),
    InitialSubscription(InitialSubscription),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct InitialSubscription {
    pub(crate) database_update: DatabaseUpdate,
    pub(crate) request_id: u64,
    pub(crate) total_host_execution_duration: TotalHostExecutionDuration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct DatabaseUpdate {
    pub(crate) tables: Vec<TempTable>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct IdentityToken {
    pub(crate) identity: Identity,
    pub(crate) token: Box<str>,
    pub(crate) connection_id: ConnectionId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Identity {
    pub(crate) __identity__: Box<str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Address {
    pub(crate) __address__: u128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectionId {
    __connection_id__: u128,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TransactionUpdate {
    pub(crate) status: Status,
    pub(crate) timestamp: Timestamp,
    pub(crate) caller_identity: Identity,
    pub(crate) caller_connection_id: ConnectionId,
    pub(crate) reducer_call: ReducerCall,
    pub(crate) energy_quanta_used: EnergyQuantaUsed,
    pub(crate) total_host_execution_duration: TotalHostExecutionDuration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TotalHostExecutionDuration {
    pub(crate) __time_duration_micros__: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Status {
    #[serde(rename = "Committed")]
    pub(crate) committed: Option<Committed>,
    #[serde(rename = "Failed")]
    pub(crate) failed: Option<Box<str>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Committed {
    pub(crate) tables: Vec<TempTable>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct QueryUpdate {
    pub(crate) deletes: Vec<Box<str>>,
    pub(crate) inserts: Vec<Box<str>>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TempTable {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) num_rows: u64,
    pub(crate) updates: Vec<QueryUpdate>,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct TableWithOriginalEventTransactionUpdate {
    pub(crate) table_id: u64,
    pub(crate) table_name: Box<str>,
    pub(crate) deletes: Vec<Box<str>>,
    pub(crate) inserts: Vec<Box<str>>,
    pub(crate) original_event: TransactionUpdate,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct Timestamp {
    #[serde(with = "time::serde::timestamp::microseconds")]
    pub(crate) __timestamp_micros_since_unix_epoch__: OffsetDateTime,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct ReducerCall {
    pub(crate) reducer_name: Box<str>,
    pub(crate) reducer_id: u64,
    pub(crate) args: serde_json::Value,
    pub(crate) request_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct EnergyQuantaUsed {
    pub(crate) quanta: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct Event {
    pub(crate) timestamp: u64,
    pub(crate) status: String,
    pub(crate) caller_identity: String,
    pub(crate) function_call: FunctionCall,
    pub(crate) energy_quanta_used: u64,
    pub(crate) message: String,
    pub(crate) caller_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct FunctionCall {
    pub(crate) reducer: String,
    pub(crate) args: String,
    pub(crate) request_id: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub(crate) struct TableRowOperation {
    pub(crate) row: Value,
    pub(crate) op: String,
}
