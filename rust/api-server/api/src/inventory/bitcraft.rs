use crate::AppState;
use crate::inventory::{InventoryDescriptionCache, resolve_pockets_for_sync};
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages};
// use chrono::DateTime;

use entity::inventory::ResolvedInventory;
// use entity::inventory_changelog::TypeOfChange;
use crate::websocket::worker_channel::{UnboundedReceiver, UnboundedSender};
use futures::TryStreamExt;
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
    message_positions: HashMap<i64, usize>,
    messages_changed: Vec<::entity::inventory_changelog::ActiveModel>,
    messages_delete: Vec<i64>,
    delete_positions: HashMap<i64, usize>,
    on_conflict: OnConflict,
    description_cache: InventoryDescriptionCache,
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
            message_positions: HashMap::new(),
            messages_changed: Vec::new(),
            messages_delete: Vec::new(),
            delete_positions: HashMap::new(),
            on_conflict,
            description_cache: InventoryDescriptionCache::default(),
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
        let mut known_ids = match async {
            let query = ::entity::inventory::Entity::find()
                .select_only()
                .column(::entity::inventory::Column::EntityId)
                .filter(::entity::inventory::Column::Region.eq(database_name))
                .into_tuple::<i64>();
            let mut rows = query.stream(&self.global_app_state.conn).await?;
            let mut ids = HashSet::new();
            while let Some(id) = rows.try_next().await? {
                ids.insert(id);
            }
            Ok::<_, sea_orm::DbErr>(ids)
            // Drop the stream and return its connection before comparisons/writes.
        }
        .await
        {
            Ok(ids) => ids,
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
        // Snapshot-local scratch space grows with the batch, not snapshot length.
        let batch_size = self.batch_size.max(1);
        let mut batch = Vec::new();
        let mut existing_ids = Vec::new();
        let mut existing = HashMap::new();
        let mut changes = Vec::new();
        loop {
            if let Err(error) =
                fill_initial_batch(&mut incoming, &mut batch, batch_size, database_name).await
            {
                tracing::error!(%error, region = database_name, "Incomplete inventory snapshot; skipping stale-row deletion");
                return;
            }
            if batch.is_empty() {
                break;
            }

            existing_ids.clear();
            existing_ids.extend(
                batch
                    .iter()
                    .map(|model| model.entity_id)
                    .filter(|id| known_ids.contains(id)),
            );
            if !existing_ids.is_empty() {
                let comparison = async {
                    let query = ::entity::inventory::Entity::find()
                        .filter(::entity::inventory::Column::Region.eq(database_name))
                        .filter(crate::websocket::initial_sync::bigint_keys_filter(
                            ::entity::inventory::Column::EntityId,
                            &existing_ids,
                        ))
                        .into_model::<super::db_row::InventoryDbRow>();
                    let mut rows = query.stream(&self.global_app_state.conn).await?;
                    while let Some(row) = rows.try_next().await? {
                        existing.insert(row.0.entity_id, row.0);
                    }
                    Ok::<_, sea_orm::DbErr>(())
                }
                .await;
                if let Err(error) = comparison {
                    // A partial/decode failure must not produce writes or deletes.
                    tracing::error!(%error, region = database_name, "Could not compare inventory batch; stopping initial reconciliation");
                    return;
                }
            }
            compare_initial_batch(&mut batch, &mut existing, &mut known_ids, &mut changes);
            if !changes.is_empty() {
                if let Err(error) = ::entity::inventory::Entity::insert_many(changes.drain(..))
                    .on_conflict(self.on_conflict.clone())
                    .exec(&self.global_app_state.conn)
                    .await
                {
                    tracing::error!(%error, region = database_name, "Could not write inventory snapshot batch; skipping stale-row deletion");
                    return;
                }
            }
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
            &mut self.description_cache,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryInsert {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        region: model.region,
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

        self.pending_upsert(model);
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

        if let Some(pockets) = resolve_pockets_for_sync(
            &model.pockets,
            &self.global_app_state.item_desc,
            &self.global_app_state.cargo_desc,
            &mut self.description_cache,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryUpdate {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        region: model.region,
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

        self.pending_upsert(model);

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
            &mut self.description_cache,
        ) {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::InventoryRemove {
                    resolved_inventory: ResolvedInventory {
                        entity_id: model.entity_id,
                        region: model.region,
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

        self.pending_delete(id);
    }

    fn pending_upsert(&mut self, model: ::entity::inventory::Model) {
        let id = model.entity_id;
        if let Some(index) = self.delete_positions.remove(&id) {
            self.messages_delete.swap_remove(index);
            if let Some(&moved_id) = self.messages_delete.get(index) {
                self.delete_positions.insert(moved_id, index);
            }
        }
        if let Some(&index) = self.message_positions.get(&id) {
            self.messages[index] = model.into_active_model();
        } else {
            self.message_positions.insert(id, self.messages.len());
            self.messages.push(model.into_active_model());
        }
    }

    fn pending_delete(&mut self, id: i64) {
        if let Some(index) = self.message_positions.remove(&id) {
            self.messages.swap_remove(index);
            if let Some(moved) = self.messages.get(index) {
                self.message_positions
                    .insert(*moved.entity_id.as_ref(), index);
            }
        }
        if !self.delete_positions.contains_key(&id) {
            self.delete_positions.insert(id, self.messages_delete.len());
            self.messages_delete.push(id);
        }
    }

    async fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages);
        self.message_positions.clear();
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
        self.delete_positions.clear();
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

async fn fill_initial_batch<I>(
    incoming: &mut I,
    batch: &mut Vec<::entity::inventory::Model>,
    batch_size: usize,
    region: entity::shared::Region,
) -> Result<(), sea_orm::DbErr>
where
    I: futures::Stream<Item = Result<InventoryState, sea_orm::DbErr>> + Unpin,
{
    while batch.len() < batch_size.max(1) {
        let Some(value) = incoming.try_next().await? else {
            break;
        };
        batch.push(
            ::entity::inventory::ModelBuilder::new(value)
                .with_region(region)
                .build(),
        );
    }
    Ok(())
}

fn compare_initial_batch(
    incoming: &mut Vec<::entity::inventory::Model>,
    existing: &mut HashMap<i64, ::entity::inventory::Model>,
    known_ids: &mut HashSet<i64>,
    changes: &mut Vec<::entity::inventory::ActiveModel>,
) {
    for model in incoming.drain(..) {
        known_ids.remove(&model.entity_id);
        if existing.remove(&model.entity_id).as_ref() != Some(&model) {
            changes.push(model.into_active_model());
        }
    }
    // Release all existing pocket payloads before awaiting the INSERT, retaining
    // only the lookup map's bounded backing allocation for the next batch.
    existing.clear();
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
    use sea_orm::{DatabaseConnection, DbBackend, DbErr, MockDatabase};

    fn reconcile_initial_batch(
        mut incoming: Vec<Model>,
        existing: Vec<Model>,
        known_ids: &mut HashSet<i64>,
    ) -> Vec<::entity::inventory::ActiveModel> {
        let mut existing = existing
            .into_iter()
            .map(|model| (model.entity_id, model))
            .collect();
        let mut changes = Vec::new();
        compare_initial_batch(&mut incoming, &mut existing, known_ids, &mut changes);
        changes
    }

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

    fn raw_inventory(id: u64) -> InventoryState {
        use game_module::module_bindings as raw;
        InventoryState {
            entity_id: id,
            pockets: vec![raw::Pocket {
                volume: 20,
                contents: Some(raw::ItemStack {
                    item_id: 1,
                    quantity: 5,
                    item_type: raw::ItemType::Item,
                    durability: Some(100),
                }),
                locked: false,
            }],
            inventory_index: 0,
            cargo_index: 0,
            owner_entity_id: 10,
            player_owner_entity_id: 20,
        }
    }

    fn key_row(id: i64) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [("0".to_owned(), id.into())].into()
    }

    fn db_row(model: &Model) -> std::collections::BTreeMap<String, sea_orm::Value> {
        [
            ("entity_id".to_owned(), model.entity_id.into()),
            (
                "pockets".to_owned(),
                serde_json::to_value(&model.pockets).unwrap().into(),
            ),
            ("inventory_index".to_owned(), model.inventory_index.into()),
            ("cargo_index".to_owned(), model.cargo_index.into()),
            ("owner_entity_id".to_owned(), model.owner_entity_id.into()),
            (
                "player_owner_entity_id".to_owned(),
                model.player_owner_entity_id.into(),
            ),
            ("region".to_owned(), model.region.into()),
        ]
        .into()
    }

    fn snapshot_worker(
        db: DatabaseConnection,
        batch_size: usize,
    ) -> (
        InventoryStateWorker,
        tokio::sync::mpsc::Receiver<InventoryDbOperation>,
    ) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let mut worker = InventoryStateWorker::new(state, batch_size, Duration::from_millis(200));
        // Keep stale deletes observable without running them asynchronously.
        let (db_tx, db_rx) = channel(5);
        worker.db_tx = db_tx;
        (worker, db_rx)
    }

    #[tokio::test]
    async fn snapshot_streams_typed_comparisons_and_queues_only_region_scoped_stale_ids() {
        let unchanged = inventory(1);
        let mut changed = inventory(2);
        changed.pockets[0].contents.as_mut().unwrap().quantity = 10;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1), key_row(2), key_row(9)]])
            .append_query_results([
                vec![db_row(&unchanged), db_row(&inventory(2))],
                vec![db_row(&changed)],      // first upsert
                vec![db_row(&inventory(3))], // new-only second batch skips comparison
            ])
            .into_connection();
        let (mut worker, mut deletes) = snapshot_worker(db.clone(), 2);
        let mut changed_raw = raw_inventory(2);
        changed_raw.pockets[0].contents.as_mut().unwrap().quantity = 10;
        worker
            .handle_initial(
                vec![raw_inventory(1), changed_raw, raw_inventory(3)],
                1,
                None,
            )
            .await;
        let InventoryDbOperation::DeleteForRegion { ids, region } = deletes.try_recv().unwrap()
        else {
            panic!("expected region-scoped stale deletes");
        };
        assert_eq!(ids, [9]);
        assert_eq!(region, 1);
        assert!(deletes.try_recv().is_err());
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 4, "key read, comparison, two batched writes");
        assert!(format!("{:?}", log[2]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[3]).contains("INSERT INTO"));
    }

    #[tokio::test]
    async fn partial_key_decode_failure_never_compares_writes_or_queues_deletes() {
        let mut invalid = key_row(2);
        invalid.insert("0".to_owned(), "not an ID".into());
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1), invalid]])
            .into_connection();
        let (mut worker, mut deletes) = snapshot_worker(db.clone(), 2);
        worker.handle_initial(vec![raw_inventory(1)], 1, None).await;
        assert!(deletes.try_recv().is_err());
        assert_eq!(db.into_transaction_log().len(), 1);
    }

    #[tokio::test]
    async fn partial_typed_json_decode_failure_never_writes_or_queues_deletes() {
        let mut invalid = db_row(&inventory(2));
        invalid.insert(
            "pockets".to_owned(),
            serde_json::json!({"invalid": true}).into(),
        );
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(1), key_row(2), key_row(9)]])
            .append_query_results([vec![db_row(&inventory(1)), invalid]])
            .into_connection();
        let (mut worker, mut deletes) = snapshot_worker(db.clone(), 2);
        worker
            .handle_initial(vec![raw_inventory(1), raw_inventory(2)], 1, None)
            .await;
        assert!(deletes.try_recv().is_err());
        assert_eq!(db.into_transaction_log().len(), 2);
    }

    #[tokio::test]
    async fn failed_snapshot_upsert_stops_before_later_batches_and_stale_deletes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![key_row(9)]])
            .append_query_errors([DbErr::Custom("write failed".into())])
            .into_connection();
        let (mut worker, mut deletes) = snapshot_worker(db.clone(), 1);
        worker
            .handle_initial(vec![raw_inventory(1), raw_inventory(2)], 1, None)
            .await;
        assert!(deletes.try_recv().is_err());
        assert_eq!(
            db.into_transaction_log().len(),
            2,
            "only keys and the failed upsert"
        );
    }

    #[tokio::test]
    async fn zero_batch_size_still_processes_rows_one_at_a_time() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([
                Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new(),
            ])
            .append_query_results([vec![db_row(&inventory(1))], vec![db_row(&inventory(2))]])
            .into_connection();
        let (mut worker, mut deletes) = snapshot_worker(db.clone(), 0);
        worker
            .handle_initial(vec![raw_inventory(1), raw_inventory(2)], 1, None)
            .await;
        assert!(deletes.try_recv().is_err());
        assert_eq!(db.into_transaction_log().len(), 3);
    }

    #[test]
    fn comparison_drains_and_clears_rows_but_reuses_scratch_allocations() {
        let mut incoming = Vec::with_capacity(2);
        let mut existing = HashMap::with_capacity(2);
        let mut changes = Vec::with_capacity(2);
        let mut known_ids = HashSet::from([1, 2, 3, 4, 9]);
        let input_storage = incoming.as_ptr();
        let change_storage = changes.as_ptr();
        let map_capacity = existing.capacity();
        for first_id in [1, 3] {
            incoming.extend([inventory(first_id), inventory(first_id + 1)]);
            existing.insert(first_id, inventory(first_id));
            // This unrelated full row must also be released before writes.
            existing.insert(99, inventory(99));
            compare_initial_batch(&mut incoming, &mut existing, &mut known_ids, &mut changes);
            assert!(incoming.is_empty() && existing.is_empty());
            assert_eq!(changes, [inventory(first_id + 1).into_active_model()]);
            assert_eq!(incoming.as_ptr(), input_storage);
            assert_eq!(changes.as_ptr(), change_storage);
            assert_eq!(existing.capacity(), map_capacity);
            // The actual writer also consumes with drain rather than taking the Vec.
            drop(changes.drain(..));
        }
        assert_eq!(known_ids, HashSet::from([9]));
    }

    #[tokio::test]
    async fn partial_incoming_error_is_propagated_before_comparing_the_batch() {
        let mut incoming = futures::stream::iter([
            Ok(raw_inventory(1)),
            Ok(raw_inventory(2)),
            Ok(raw_inventory(3)),
            Err(DbErr::Custom("snapshot interrupted".into())),
            Ok(raw_inventory(4)),
        ]);
        let mut batch = Vec::with_capacity(2);
        let allocation = batch.as_ptr();
        fill_initial_batch(&mut incoming, &mut batch, 2, 1)
            .await
            .unwrap();
        assert_eq!(batch, [inventory(1), inventory(2)]);
        batch.clear();
        assert_eq!(
            fill_initial_batch(&mut incoming, &mut batch, 2, 1)
                .await
                .unwrap_err(),
            DbErr::Custom("snapshot interrupted".into())
        );
        assert_eq!(
            batch,
            [inventory(3)],
            "partial batch must not be reported complete"
        );
        assert_eq!(batch.as_ptr(), allocation);
        assert_eq!(incoming.try_next().await.unwrap().unwrap().entity_id, 4);
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
        worker.pending_upsert(inventory(1));
        let allocation = worker.messages.as_ptr();
        worker.flush_messages().await;
        assert_eq!(worker.messages.capacity(), 0);
        assert!(worker.message_positions.is_empty());
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

        worker.pending_delete(1);
        worker.pending_delete(2);
        let allocation = worker.messages_delete.as_ptr();
        worker.flush_deletes().await;
        assert_eq!(worker.messages_delete.capacity(), 0);
        assert!(worker.delete_positions.is_empty());
        let InventoryDbOperation::Delete(ids) = db_rx.recv().await.unwrap() else {
            panic!("expected deletes")
        };
        assert_eq!(ids.as_ptr(), allocation);
        assert_eq!(ids, [1, 2]);

        // A later batch still works, without a preallocated replacement buffer.
        worker.pending_upsert(inventory(3));
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

    #[tokio::test]
    async fn indexed_pending_operations_match_latest_primary_key_state_and_repair_swaps() {
        let (mut worker, _db) = snapshot_worker(DatabaseConnection::default(), 3000);
        let mut expected = HashMap::new();
        for round in 0..512i64 {
            let id = (round * 17) % 23;
            if round % 5 == 0 {
                worker.pending_delete(id);
                worker.pending_delete(id); // repeated removes must not leave stale duplicates
                expected.insert(id, None);
            } else {
                let mut model = inventory(id);
                model.region = (round % 3) as i16;
                model.inventory_index = round as i32;
                worker.pending_upsert(model.clone());
                expected.insert(id, Some(model.into_active_model()));
            }
            assert_eq!(worker.message_positions.len(), worker.messages.len());
            assert_eq!(worker.delete_positions.len(), worker.messages_delete.len());
            for (index, row) in worker.messages.iter().enumerate() {
                let id = *row.entity_id.as_ref();
                assert_eq!(worker.message_positions.get(&id), Some(&index));
                assert_eq!(expected[&id].as_ref(), Some(row));
                assert!(!worker.delete_positions.contains_key(&id));
            }
            for (index, id) in worker.messages_delete.iter().enumerate() {
                assert_eq!(worker.delete_positions.get(id), Some(&index));
                assert_eq!(expected[id], None);
                assert!(!worker.message_positions.contains_key(id));
            }
            assert_eq!(
                worker.messages.len() + worker.messages_delete.len(),
                expected.len()
            );
        }
    }

    #[tokio::test]
    async fn persistence_coalesces_without_dropping_notifications_or_changelog_entries() {
        use crate::inventory::resolution_tests::{cargo_description, item_description};
        let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(DatabaseConnection::default(), tx, Default::default());
        state.item_desc.insert(1, item_description());
        state.cargo_desc.insert(2, cargo_description());
        let mut worker = InventoryStateWorker::new(state, 100, Duration::from_secs(60));
        let (db_tx, mut operations) = channel(5);
        worker.db_tx = db_tx;
        let first = ::entity::inventory_changelog::ActiveModel {
            entity_id: sea_orm::Set(1),
            pocket_number: sea_orm::Set(0),
            ..Default::default()
        };
        let second = ::entity::inventory_changelog::ActiveModel {
            entity_id: sea_orm::Set(1),
            pocket_number: sea_orm::Set(1),
            ..Default::default()
        };
        worker
            .messages_changed
            .extend([first.clone(), second.clone()]);
        worker.handle_insert(raw_inventory(1), 1).await;
        worker.handle_insert(raw_inventory(2), 1).await;
        let mut updated = raw_inventory(1);
        updated.pockets[0].contents.as_mut().unwrap().quantity = 7;
        worker
            .handle_update(updated.clone(), raw_inventory(1), 2)
            .await;
        worker.handle_remove(updated.clone(), 2).await;
        worker.handle_remove(updated.clone(), 2).await;
        updated.pockets[0].contents.as_mut().unwrap().quantity = 9;
        worker.handle_insert(updated.clone(), 3).await;
        worker.handle_remove(raw_inventory(2), 1).await;

        let mut observed = Vec::new();
        while let Ok(event) = notifications.try_recv() {
            let (kind, inventory) = match event {
                OutboundWebSocketMessages::InventoryInsert {
                    resolved_inventory, ..
                } => ("insert", resolved_inventory),
                OutboundWebSocketMessages::InventoryUpdate { resolved_inventory } => {
                    ("update", resolved_inventory)
                }
                OutboundWebSocketMessages::InventoryRemove { resolved_inventory } => {
                    ("remove", resolved_inventory)
                }
                _ => panic!("unexpected notification"),
            };
            observed.push((
                kind,
                inventory.entity_id,
                inventory.pockets[0].contents.as_ref().unwrap().quantity,
            ));
        }
        assert_eq!(
            observed,
            [
                ("insert", 1, 5),
                ("insert", 2, 5),
                ("update", 1, 7),
                ("remove", 1, 7),
                ("remove", 1, 7),
                ("insert", 1, 9),
                ("remove", 2, 5)
            ]
        );
        worker.flush().await;
        match operations.try_recv().unwrap() {
            InventoryDbOperation::Upsert(rows) => assert_eq!(
                rows,
                [::entity::inventory::ModelBuilder::new(updated)
                    .with_region(3)
                    .build()
                    .into_active_model()]
            ),
            _ => panic!("upserts must precede changelog and deletes"),
        }
        match operations.try_recv().unwrap() {
            InventoryDbOperation::UpsertChangelog(rows) => assert_eq!(rows, [first, second]),
            _ => panic!("changelog must not be coalesced"),
        }
        match operations.try_recv().unwrap() {
            InventoryDbOperation::Delete(ids) => assert_eq!(ids, [2]),
            _ => panic!("expected final delete"),
        }
        assert!(worker.is_idle());
        assert!(worker.message_positions.is_empty() && worker.delete_positions.is_empty());
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
