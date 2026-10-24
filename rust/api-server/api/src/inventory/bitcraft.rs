use crate::AppState;
use crate::inventory::resolve_pockets_for_sync;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages};
// use chrono::DateTime;

use entity::inventory::ResolvedInventory;
// use entity::inventory_changelog::TypeOfChange;
use crate::websocket::worker_channel::{UnboundedReceiver, UnboundedSender};
use futures::{StreamExt, TryStreamExt};
use game_module::module_bindings::InventoryState;
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, QuerySelect};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::mpsc::{Sender, channel};

enum InventoryDbOperation {
    Upsert(Vec<::entity::inventory::ActiveModel>),
    UpsertChangelog(Vec<::entity::inventory_changelog::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct InventoryStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<InventoryState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<InventoryState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<InventoryDbOperation>,
    messages: Vec<::entity::inventory::ActiveModel>,
    messages_changed: Vec<::entity::inventory_changelog::ActiveModel>,
    messages_delete: Vec<i64>,
    on_conflict: OnConflict,
}

impl InventoryStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_inventory_state_db_worker(global_app_state.clone());
        let (tx, rx) =
            crate::websocket::worker_channel::shutdown_channel(&global_app_state.shutdown);
        let on_conflict = sea_query::OnConflict::column(::entity::inventory::Column::EntityId)
            .update_columns([
                ::entity::inventory::Column::Pockets,
                ::entity::inventory::Column::InventoryIndex,
                ::entity::inventory::Column::CargoIndex,
                ::entity::inventory::Column::OwnerEntityId,
                ::entity::inventory::Column::PlayerOwnerEntityId,
                ::entity::inventory::Column::Region,
            ])
            .to_owned();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::new(),
            messages_changed: Vec::new(),
            messages_delete: Vec::new(),
            on_conflict,
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::inventory::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(InventoryDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue InventoryState upserts"
            );
        }
    }

    async fn queue_changelog_upserts(
        &self,
        messages: Vec<::entity::inventory_changelog::ActiveModel>,
    ) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(InventoryDbOperation::UpsertChangelog(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue InventoryChangelog upserts"
            );
        }
    }

    async fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(InventoryDbOperation::Delete(ids)).await {
            tracing::error!(
                error = error.to_string(),
                "Could not queue InventoryState deletes"
            );
        }
    }

    async fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(InventoryDbOperation::DeleteForRegion { ids, region })
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue InventoryState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<InventoryState>) {
        match msg {
            SpacetimeUpdateMessages::Initial {
                data,
                database_name,
                batches,
            } => {
                self.handle_initial(data, database_name, batches).await;
            }
            SpacetimeUpdateMessages::Insert {
                new, database_name, ..
            } => {
                self.handle_insert(new, database_name).await;
            }
            SpacetimeUpdateMessages::Update {
                new,
                old,
                database_name,
                ..
            } => {
                self.handle_update(new, old, database_name).await;
            }
            SpacetimeUpdateMessages::Remove {
                delete,
                database_name,
                ..
            } => {
                self.handle_remove(delete, database_name).await;
            }
        }
    }

    async fn handle_initial(
        &mut self,
        data: Vec<InventoryState>,
        database_name: entity::shared::Region,
        batches: Option<crate::websocket::initial_batches::InitialBatches<InventoryState>>,
    ) {
        tracing::debug!("Count of inventory amount to work on {}", data.len());
        // Keep only IDs for stale-row detection, not another region-wide copy of
        // every inventory's pockets. Full existing rows are fetched one batch at a time.
        let mut known_ids = match ::entity::inventory::Entity::find()
            .select_only()
            .column(::entity::inventory::Column::EntityId)
            .filter(::entity::inventory::Column::Region.eq(database_name))
            .into_tuple::<i64>()
            .all(&self.global_app_state.conn)
            .await
        {
            Ok(ids) => ids.into_iter().collect::<HashSet<_>>(),
            Err(error) => {
                tracing::error!(
                    %error,
                    region = database_name,
                    "Could not load inventory IDs; skipping initial reconciliation"
                );
                return;
            }
        };

        let incoming = crate::websocket::initial_batches::rows(data, batches);
        futures::pin_mut!(incoming);
        loop {
            let batch = incoming
                .by_ref()
                .take(self.batch_size.max(1))
                .map_ok(|value| {
                    ::entity::inventory::ModelBuilder::new(value)
                        .with_region(database_name)
                        .build()
                })
                .try_collect::<Vec<_>>()
                .await;
            let batch = match batch {
                Ok(batch) => batch,
                Err(error) => {
                    tracing::error!(%error, region = database_name, "Incomplete inventory snapshot; skipping stale-row deletion");
                    return;
                }
            };
            if batch.is_empty() {
                break;
            }

            let existing_ids = batch
                .iter()
                .map(|model| model.entity_id)
                .filter(|id| known_ids.contains(id))
                .collect::<Vec<_>>();
            let existing = if existing_ids.is_empty() {
                // A fresh region/all-new batch needs no pocket data read at all.
                Vec::new()
            } else {
                match ::entity::inventory::Entity::find()
                    .filter(::entity::inventory::Column::Region.eq(database_name))
                    .filter(::entity::inventory::Column::EntityId.is_in(existing_ids))
                    .into_model::<super::db_row::InventoryDbRow>()
                    .all(&self.global_app_state.conn)
                    .await
                {
                    Ok(existing) => existing.into_iter().map(|row| row.0).collect(),
                    Err(error) => {
                        // Never delete stale rows after an incomplete comparison.
                        tracing::error!(%error, region = database_name, "Could not compare inventory batch; stopping initial reconciliation");
                        return;
                    }
                }
            };
            let mut changes = reconcile_initial_batch(batch, existing, &mut known_ids);
            insert_multiple_inventory(&self.global_app_state, &self.on_conflict, &mut changes)
                .await;
        }

        tracing::debug!("Count of inventory amount to delete {}", known_ids.len());

        let mut stale_ids = known_ids.into_iter();
        loop {
            let ids = stale_ids.by_ref().take(1_000).collect::<Vec<_>>();
            if ids.is_empty() {
                break;
            }
            self.queue_region_deletes(ids, database_name).await;
        }
    }

    async fn handle_insert(&mut self, new: InventoryState, database_name: entity::shared::Region) {
        let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(new)
            .with_region(database_name)
            .build();

        let mut player_owner_id = model.player_owner_entity_id;
        let mut nickname = None;

        if model.player_owner_entity_id == 0
            && let Some(deployable_state) = self
                .global_app_state
                .deployable_state
                .get(&model.owner_entity_id)
        {
            player_owner_id = deployable_state.owner_id;
            nickname = Some(deployable_state.nickname.clone());
        }

        if let Some(pockets) = resolve_pockets_for_sync(
            &model.pockets,
            &self.global_app_state.item_desc,
            &self.global_app_state.cargo_desc,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryInsert {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        pockets,
                        inventory_index: model.inventory_index,
                        cargo_index: model.cargo_index,
                        owner_entity_id: model.owner_entity_id,
                        player_owner_entity_id: model.player_owner_entity_id,
                        nickname,
                        claim: None,
                    },
                    player_owner_id,
                });
        }

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        if let Some(index) =
            self.messages
                .iter()
                .position(|value: &::entity::inventory::ActiveModel| {
                    value.entity_id.as_ref() == &model.entity_id
                })
        {
            self.messages.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(
        &mut self,
        new: InventoryState,
        _old: InventoryState,
        database_name: entity::shared::Region,
    ) {
        // let caller_identity = None;
        // let mut timestamp = None;

        // let new_model = new.clone();
        let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(new)
            .with_region(database_name)
            .build();

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        if let Some(pockets) = resolve_pockets_for_sync(
            &model.pockets,
            &self.global_app_state.item_desc,
            &self.global_app_state.cargo_desc,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryUpdate {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        pockets,
                        inventory_index: model.inventory_index,
                        cargo_index: model.cargo_index,
                        owner_entity_id: model.owner_entity_id,
                        player_owner_entity_id: model.player_owner_entity_id,
                        nickname: None,
                        claim: None,
                    },
                });
        }

        self.messages.push(model.into_active_model());

        // if let Some(caller_identity) = caller_identity {
        //     let user_id = self
        //         .global_app_state
        //         .user_state
        //         .get(&caller_identity)
        //         .map(|entity_id| entity_id.to_owned() as i64);
        //     for (pocket_index, new_pocket) in new_model.pockets.iter().enumerate() {
        //         if pocket_index >= old.pockets.len() {
        //             tracing::warn!(
        //                 "Inventory new pocket amount is less then before ?!? Player {}, EntityId {}, OwnerEntityId {}, Pockets New {}, Pockets Old {} :: {} {}",
        //                 new_model.player_owner_entity_id,
        //                 new_model.entity_id,
        //                 new_model.owner_entity_id,
        //                 new_model.pockets.len(),
        //                 old.pockets.len(),
        //                 old.pockets.len(),
        //                 pocket_index
        //             );
        //             break;
        //         }
        //
        //         let old_pocket = &old.pockets[pocket_index];
        //
        //         let new_item_id = new_pocket.contents.as_ref().map(|c| c.item_id);
        //         let new_item_type = new_pocket.contents.as_ref().map(|c| c.item_type.into());
        //         let new_item_quantity = new_pocket.contents.as_ref().map(|c| c.quantity);
        //
        //         let old_item_id = old_pocket.contents.as_ref().map(|c| c.item_id);
        //         let old_item_type = old_pocket.contents.as_ref().map(|c| c.item_type.into());
        //         let old_item_quantity = old_pocket.contents.as_ref().map(|c| c.quantity);
        //
        //         if new_item_id == old_item_id
        //             && new_item_type == old_item_type
        //             && new_item_quantity == old_item_quantity
        //         {
        //             continue;
        //         }
        //
        //         let type_of_change = match (old_item_id, new_item_id) {
        //             (Some(_), None) => TypeOfChange::Remove,
        //             (None, Some(_)) => TypeOfChange::Add,
        //             (Some(old), Some(new)) => {
        //                 if old != new {
        //                     TypeOfChange::AddAndRemove
        //                 } else {
        //                     TypeOfChange::Update
        //                 }
        //             }
        //             _ => unreachable!("This type of change should never happen for an inventory"),
        //         };
        //
        //         self.messages_changed
        //             .push(::entity::inventory_changelog::ActiveModel {
        //                 id: NotSet,
        //                 entity_id: Set(new_model.entity_id as i64),
        //                 user_id: Set(user_id),
        //                 pocket_number: Set(pocket_index as i32),
        //                 old_item_id: Set(old_item_id),
        //                 old_item_type: Set(old_item_type),
        //                 old_item_quantity: Set(old_item_quantity),
        //                 new_item_id: Set(new_item_id),
        //                 new_item_type: Set(new_item_type),
        //                 new_item_quantity: Set(new_item_quantity),
        //                 type_of_change: Set(type_of_change),
        //                 timestamp: Set(DateTime::from_timestamp_micros(
        //                     timestamp.unwrap().to_micros_since_unix_epoch(),
        //                 )
        //                 .unwrap()),
        //             })
        //     }
        // }

        if self.messages_changed.len() >= self.batch_size {
            self.flush_changes().await;
        }
    }

    async fn handle_remove(
        &mut self,
        delete: InventoryState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(delete)
            .with_region(database_name)
            .build();
        let id = model.entity_id;

        if let Some(pockets) = resolve_pockets_for_sync(
            &model.pockets,
            &self.global_app_state.item_desc,
            &self.global_app_state.cargo_desc,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryRemove {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        pockets,
                        inventory_index: model.inventory_index,
                        cargo_index: model.cargo_index,
                        owner_entity_id: model.owner_entity_id,
                        player_owner_entity_id: model.player_owner_entity_id,
                        nickname: None,
                        claim: None,
                    },
                });
        }

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }
        self.messages_delete.push(id);
    }

    async fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages);
        self.queue_upserts(messages).await;
    }

    async fn flush_changes(&mut self) {
        if self.messages_changed.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages_changed);
        self.queue_changelog_upserts(messages).await;
    }

    async fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("InventoryState::Remove");
        let messages_delete = std::mem::take(&mut self.messages_delete);
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for InventoryStateWorker {
    type Entity = InventoryState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "inventory_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size
            || self.messages_changed.len() >= self.batch_size
            || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty()
            && self.messages_changed.is_empty()
            && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages().await;
        self.flush_changes().await;
        self.flush_deletes().await;
    }
}

fn reconcile_initial_batch(
    incoming: Vec<::entity::inventory::Model>,
    existing: Vec<::entity::inventory::Model>,
    known_ids: &mut HashSet<i64>,
) -> Vec<::entity::inventory::ActiveModel> {
    let mut existing = existing
        .into_iter()
        .map(|model| (model.entity_id, model))
        .collect::<HashMap<_, _>>();
    incoming
        .into_iter()
        .filter_map(|model| {
            known_ids.remove(&model.entity_id);
            if existing.remove(&model.entity_id).as_ref() == Some(&model) {
                None
            } else {
                Some(model.into_active_model())
            }
        })
        .collect()
}

fn start_inventory_state_db_worker(global_app_state: AppState) -> Sender<InventoryDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict = sea_query::OnConflict::column(::entity::inventory::Column::EntityId)
        .update_columns([
            ::entity::inventory::Column::Pockets,
            ::entity::inventory::Column::InventoryIndex,
            ::entity::inventory::Column::CargoIndex,
            ::entity::inventory::Column::OwnerEntityId,
            ::entity::inventory::Column::PlayerOwnerEntityId,
            ::entity::inventory::Column::Region,
        ])
        .to_owned();
    let on_conflict_changelog = sea_query::OnConflict::columns([
        ::entity::inventory_changelog::Column::Id,
        ::entity::inventory_changelog::Column::Timestamp,
    ])
    .update_columns([
        ::entity::inventory_changelog::Column::EntityId,
        ::entity::inventory_changelog::Column::UserId,
        ::entity::inventory_changelog::Column::PocketNumber,
        ::entity::inventory_changelog::Column::OldItemId,
        ::entity::inventory_changelog::Column::OldItemType,
        ::entity::inventory_changelog::Column::OldItemQuantity,
        ::entity::inventory_changelog::Column::NewItemId,
        ::entity::inventory_changelog::Column::NewItemType,
        ::entity::inventory_changelog::Column::NewItemQuantity,
        ::entity::inventory_changelog::Column::TypeOfChange,
        ::entity::inventory_changelog::Column::Timestamp,
    ])
    .to_owned();

    global_app_state.shutdown.clone().spawn_worker(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                InventoryDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_inventory(&global_app_state, &on_conflict, &mut messages).await;
                }
                InventoryDbOperation::UpsertChangelog(messages) => {
                    let mut messages = messages;
                    insert_multiple_inventory_changelog(
                        &global_app_state,
                        &on_conflict_changelog,
                        &mut messages,
                    )
                    .await;
                }
                InventoryDbOperation::Delete(ids) => {
                    delete_multiple_inventory(&global_app_state, ids).await;
                }
                InventoryDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_inventory_for_region(&global_app_state, ids, region).await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_inventory(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::inventory::Entity::delete_many()
            .filter(::entity::inventory::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                InventoryState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete InventoryState"
            );
        }
    }
}

async fn delete_multiple_inventory_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::inventory::Entity::delete_many()
            .filter(::entity::inventory::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::inventory::Column::Region.eq(region))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                Inventory = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete Inventory"
            );
        }
    }
}

async fn insert_multiple_inventory(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::inventory::ActiveModel>,
) {
    if messages.is_empty() {
        return;
    }

    let insert = ::entity::inventory::Entity::insert_many(std::mem::take(messages))
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(e) = insert {
        tracing::error!("Error inserting InventoryState chunk: {}", e);
    }
}

async fn insert_multiple_inventory_changelog(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::inventory_changelog::ActiveModel>,
) {
    if messages.is_empty() {
        return;
    }

    let insert = ::entity::inventory_changelog::Entity::insert_many(messages.iter().cloned())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(e) = insert {
        tracing::error!("Error inserting InventoryChangelog chunk: {}", e);
    }

    messages.clear();
}

#[cfg(test)]
mod initial_reconciliation_tests {
    use super::*;
    use entity::inventory::{ItemStack, ItemType, Model, Pocket};

    fn inventory(id: i64) -> Model {
        Model {
            entity_id: id,
            pockets: vec![Pocket {
                volume: 20,
                contents: Some(ItemStack {
                    item_id: 1,
                    quantity: 5,
                    item_type: ItemType::Item,
                    durability: Some(100),
                }),
                locked: false,
            }],
            inventory_index: 0,
            cargo_index: 0,
            owner_entity_id: 10,
            player_owner_entity_id: 20,
            region: 1,
        }
    }

    #[test]
    fn only_new_and_changed_rows_are_upserted_and_missing_ids_remain() {
        let unchanged = inventory(1);
        let mut changed = inventory(2);
        changed.pockets[0].contents.as_mut().unwrap().quantity = 10;
        let mut known_ids = HashSet::from([1, 2, 3]);

        let changes = reconcile_initial_batch(
            vec![unchanged.clone(), changed.clone(), inventory(4)],
            vec![unchanged, inventory(2)],
            &mut known_ids,
        );

        assert_eq!(
            changes,
            vec![
                changed.into_active_model(),
                inventory(4).into_active_model()
            ]
        );
        assert_eq!(known_ids, HashSet::from([3]));
    }

    #[tokio::test]
    async fn flushing_transfers_allocations_and_leaves_zero_capacity_buffers() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let mut worker = InventoryStateWorker::new(state, 3_000, Duration::from_millis(200));
        // Capture queue operations rather than executing database writes.
        let (db_tx, mut db_rx) = channel(5);
        worker.db_tx = db_tx;
        let model = inventory(1).into_active_model();
        worker.messages.push(model.clone());
        let allocation = worker.messages.as_ptr();
        worker.flush_messages().await;
        assert_eq!(worker.messages.capacity(), 0);
        let InventoryDbOperation::Upsert(rows) = db_rx.recv().await.unwrap() else {
            panic!("expected upsert")
        };
        assert_eq!(rows.as_ptr(), allocation);
        assert_eq!(rows, [model]);

        worker.messages_changed.push(Default::default());
        let allocation = worker.messages_changed.as_ptr();
        worker.flush_changes().await;
        assert_eq!(worker.messages_changed.capacity(), 0);
        let InventoryDbOperation::UpsertChangelog(rows) = db_rx.recv().await.unwrap() else {
            panic!("expected changelog")
        };
        assert_eq!(rows.as_ptr(), allocation);
        assert_eq!(rows.len(), 1);

        worker.messages_delete.extend([1, 2]);
        let allocation = worker.messages_delete.as_ptr();
        worker.flush_deletes().await;
        assert_eq!(worker.messages_delete.capacity(), 0);
        let InventoryDbOperation::Delete(ids) = db_rx.recv().await.unwrap() else {
            panic!("expected deletes")
        };
        assert_eq!(ids.as_ptr(), allocation);
        assert_eq!(ids, [1, 2]);

        // A later batch still works, without a preallocated replacement buffer.
        worker.messages.push(inventory(3).into_active_model());
        worker.flush_messages().await;
        assert_eq!(worker.messages.capacity(), 0);
        let InventoryDbOperation::Upsert(rows) = db_rx.recv().await.unwrap() else {
            panic!("expected next upsert")
        };
        assert_eq!(rows, [inventory(3).into_active_model()]);
        worker.flush_messages().await;
        worker.flush_changes().await;
        worker.flush_deletes().await;
        assert!(db_rx.try_recv().is_err());
    }

    #[test]
    fn metadata_and_pocket_changes_are_not_ignored() {
        let original = inventory(1);
        let mut variants = vec![original.clone(); 7];
        variants[0].inventory_index += 1;
        variants[1].cargo_index += 1;
        variants[2].owner_entity_id += 1;
        variants[3].player_owner_entity_id += 1;
        variants[4].region += 1;
        variants[5].pockets[0].locked = true;
        variants[6].pockets[0].contents = None;

        for changed in variants {
            let changes = reconcile_initial_batch(
                vec![changed.clone()],
                vec![original.clone()],
                &mut HashSet::from([1]),
            );
            assert_eq!(changes, vec![changed.into_active_model()]);
        }
    }

    #[test]
    fn batched_comparison_matches_whole_snapshot_comparison() {
        let existing = (0..200).map(inventory).collect::<Vec<_>>();
        let incoming = (20..220)
            .map(|id| {
                let mut model = inventory(id);
                if id % 3 == 0 {
                    model.pockets[0].volume += 1;
                }
                model
            })
            .collect::<Vec<_>>();

        let mut whole_snapshot = existing
            .iter()
            .cloned()
            .map(|model| (model.entity_id, model))
            .collect::<HashMap<_, _>>();
        let expected = incoming
            .iter()
            .filter_map(|model| {
                (whole_snapshot.remove(&model.entity_id).as_ref() != Some(model))
                    .then(|| model.clone().into_active_model())
            })
            .collect::<Vec<_>>();
        let expected_deletes = whole_snapshot.into_keys().collect::<HashSet<_>>();

        for batch_size in [1, 7, 100, 3_000] {
            let mut known_ids = existing.iter().map(|model| model.entity_id).collect();
            let mut changes = Vec::new();
            for batch in incoming.chunks(batch_size) {
                let batch_ids = batch
                    .iter()
                    .map(|model| model.entity_id)
                    .collect::<HashSet<_>>();
                let existing_batch = existing
                    .iter()
                    .filter(|model| batch_ids.contains(&model.entity_id))
                    .cloned()
                    .collect();
                changes.extend(reconcile_initial_batch(
                    batch.to_vec(),
                    existing_batch,
                    &mut known_ids,
                ));
            }
            assert_eq!(changes, expected);
            assert_eq!(known_ids, expected_deletes);
        }
    }

    #[test]
    fn empty_snapshot_leaves_all_existing_ids_for_deletion() {
        let mut known_ids = HashSet::from([1, 2]);
        assert!(reconcile_initial_batch(vec![], vec![], &mut known_ids).is_empty());
        assert_eq!(known_ids, HashSet::from([1, 2]));
    }
}
