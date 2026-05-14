use crate::AppState;
use crate::inventory::resolve_pocket;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use chrono::DateTime;

use entity::inventory::ResolvedInventory;
use entity::inventory_changelog::TypeOfChange;
use game_module::module_bindings::InventoryState;
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, NotSet, Set};
use spacetimedb_sdk::__codegen::Reducer;
use spacetimedb_sdk::Event;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{
    Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel,
};

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
        let (tx, rx) = unbounded_channel();
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
            messages: Vec::with_capacity(batch_size + 10),
            messages_changed: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
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
                ..
            } => {
                self.handle_initial(data, database_name).await;
            }
            SpacetimeUpdateMessages::Insert {
                new, database_name, ..
            } => {
                self.handle_insert(new, database_name).await;
            }
            SpacetimeUpdateMessages::Update {
                new,
                old,
                event,
                database_name,
                ..
            } => {
                self.handle_update(new, old, event, database_name).await;
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
    ) {
        tracing::debug!("Count of inventory amount to work on {}", data.len());
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_inventory = ::entity::inventory::Entity::find()
            .filter(::entity::inventory::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole inventory state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(value)
                .with_region(database_name)
                .build();

            model
        }) {
            use std::collections::hash_map::Entry;
            match currently_known_inventory.entry(model.entity_id) {
                Entry::Occupied(entry) => {
                    let existing_model = entry.get();
                    if &model != existing_model {
                        local_messages.push(model.into_active_model());
                    }
                    entry.remove();
                }
                Entry::Vacant(_entry) => {
                    local_messages.push(model.into_active_model());
                }
            }
            if local_messages.len() >= self.batch_size {
                insert_multiple_inventory(
                    &self.global_app_state,
                    &self.on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_inventory(
                &self.global_app_state,
                &self.on_conflict,
                &mut local_messages,
            )
            .await;
        }

        tracing::debug!(
            "Count of inventory amount to delete {}",
            currently_known_inventory.len()
        );

        self.queue_region_deletes(
            currently_known_inventory.into_keys().collect(),
            database_name,
        )
        .await;
    }

    async fn handle_insert(&mut self, new: InventoryState, database_name: entity::shared::Region) {
        let model: ::entity::inventory::Model = ::entity::inventory::ModelBuilder::new(new)
            .with_region(database_name)
            .build();

        let mut pockets = vec![];
        for pocket in &model.pockets {
            pockets.push(resolve_pocket(
                pocket,
                &self.global_app_state.item_desc,
                &self.global_app_state.cargo_desc,
            ));
        }

        let mut player_owner_id = model.player_owner_entity_id;
        let mut nickname = None;

        if model.player_owner_entity_id == 0 {
            if let Some(deployable_state) = self
                .global_app_state
                .deployable_state
                .get(&model.owner_entity_id)
            {
                player_owner_id = deployable_state.owner_id;
                nickname = Some(deployable_state.nickname.clone());
            }
        }

        let _ = self
            .global_app_state
            .tx
            .send(WebSocketMessages::InventoryInsert {
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
        old: InventoryState,
        event: Option<
            Box<spacetimedb_sdk::__codegen::Event<game_module::module_bindings::Reducer>>,
        >,
        database_name: entity::shared::Region,
    ) {
        let mut caller_identity = None;
        let mut timestamp = None;
        if let Some(event) = &event {
            if let Event::Reducer(event) = &**event {
                match event.reducer.reducer_name() {
                    "inventory_sort" => {}
                    _ => {
                        caller_identity = Some(event.caller_identity);
                        timestamp = Some(event.timestamp);
                    }
                }
            }
        }

        let new_model = new.clone();
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

        let mut pockets = vec![];
        for pocket in &model.pockets {
            pockets.push(resolve_pocket(
                pocket,
                &self.global_app_state.item_desc,
                &self.global_app_state.cargo_desc,
            ));
        }

        let _ = self
            .global_app_state
            .tx
            .send(WebSocketMessages::InventoryUpdate {
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

        self.messages.push(model.into_active_model());

        if let Some(caller_identity) = caller_identity {
            let user_id = self
                .global_app_state
                .user_state
                .get(&caller_identity)
                .map(|entity_id| entity_id.to_owned() as i64);
            for (pocket_index, new_pocket) in new_model.pockets.iter().enumerate() {
                if pocket_index >= old.pockets.len() {
                    tracing::warn!(
                        "Inventory new pocket amount is less then before ?!? Player {}, EntityId {}, OwnerEntityId {}, Pockets New {}, Pockets Old {} :: {} {}",
                        new_model.player_owner_entity_id,
                        new_model.entity_id,
                        new_model.owner_entity_id,
                        new_model.pockets.len(),
                        old.pockets.len(),
                        old.pockets.len(),
                        pocket_index
                    );
                    break;
                }

                let old_pocket = &old.pockets[pocket_index];

                let new_item_id = new_pocket.contents.as_ref().map(|c| c.item_id);
                let new_item_type = new_pocket.contents.as_ref().map(|c| c.item_type.into());
                let new_item_quantity = new_pocket.contents.as_ref().map(|c| c.quantity);

                let old_item_id = old_pocket.contents.as_ref().map(|c| c.item_id);
                let old_item_type = old_pocket.contents.as_ref().map(|c| c.item_type.into());
                let old_item_quantity = old_pocket.contents.as_ref().map(|c| c.quantity);

                if new_item_id == old_item_id
                    && new_item_type == old_item_type
                    && new_item_quantity == old_item_quantity
                {
                    continue;
                }

                let type_of_change = match (old_item_id, new_item_id) {
                    (Some(_), None) => TypeOfChange::Remove,
                    (None, Some(_)) => TypeOfChange::Add,
                    (Some(old), Some(new)) => {
                        if old != new {
                            TypeOfChange::AddAndRemove
                        } else {
                            TypeOfChange::Update
                        }
                    }
                    _ => unreachable!("This type of change should never happen for an inventory"),
                };

                self.messages_changed
                    .push(::entity::inventory_changelog::ActiveModel {
                        id: NotSet,
                        entity_id: Set(new_model.entity_id as i64),
                        user_id: Set(user_id),
                        pocket_number: Set(pocket_index as i32),
                        old_item_id: Set(old_item_id),
                        old_item_type: Set(old_item_type),
                        old_item_quantity: Set(old_item_quantity),
                        new_item_id: Set(new_item_id),
                        new_item_type: Set(new_item_type),
                        new_item_quantity: Set(new_item_quantity),
                        type_of_change: Set(type_of_change),
                        timestamp: Set(DateTime::from_timestamp_micros(
                            timestamp.unwrap().to_micros_since_unix_epoch(),
                        )
                        .unwrap()),
                    })
            }
        }

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

        let mut pockets = vec![];
        for pocket in &model.pockets {
            pockets.push(resolve_pocket(
                pocket,
                &self.global_app_state.item_desc,
                &self.global_app_state.cargo_desc,
            ));
        }

        let _ = self
            .global_app_state
            .tx
            .send(WebSocketMessages::InventoryRemove {
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

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages).await;
    }

    async fn flush_changes(&mut self) {
        if self.messages_changed.is_empty() {
            return;
        }

        let messages = std::mem::replace(
            &mut self.messages_changed,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_changelog_upserts(messages).await;
    }

    async fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("InventoryState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
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

    tokio::spawn(async move {
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
            .filter(::entity::inventory::Column::Region.eq(region.clone()))
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

    let insert = ::entity::inventory::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(e) = insert {
        tracing::error!("Error inserting InventoryState chunk: {}", e);
    }

    messages.clear();
}

async fn insert_multiple_inventory_changelog(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::inventory_changelog::ActiveModel>,
) {
    if messages.is_empty() {
        return;
    }

    let insert = ::entity::inventory_changelog::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(e) = insert {
        tracing::error!("Error inserting InventoryChangelog chunk: {}", e);
    }

    messages.clear();
}
