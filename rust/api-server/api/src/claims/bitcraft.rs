use crate::AppState;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use entity::{claim_local_state, claim_member_state, claim_state, claim_tech_state};
use game_module::module_bindings::{
    ClaimLocalState, ClaimMemberState, ClaimState, ClaimTechDesc, ClaimTechState,
};
use migration::{OnConflict, sea_query};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, TryIntoModel};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel};

enum ClaimStateDbOperation {
    Upsert(Vec<::entity::claim_state::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct ClaimStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ClaimState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<ClaimStateDbOperation>,
    messages: Vec<::entity::claim_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl ClaimStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_claim_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::claim_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimStateDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimState upserts"
            );
        }
    }

    async fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimStateDbOperation::Delete(ids)).await {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimState deletes"
            );
        }
    }

    async fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimStateDbOperation::DeleteForRegion { ids, region })
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ClaimState>) {
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
                new, database_name, ..
            } => {
                self.handle_update(new, database_name).await;
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
        data: Vec<ClaimState>,
        database_name: entity::shared::Region,
    ) {
        let on_conflict = sea_query::OnConflict::column(claim_state::Column::EntityId)
            .update_columns([
                claim_state::Column::OwnerPlayerEntityId,
                claim_state::Column::OwnerBuildingEntityId,
                claim_state::Column::Name,
                claim_state::Column::Neutral,
                claim_state::Column::Region,
            ])
            .to_owned();
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_claim_state = ::entity::claim_state::Entity::find()
            .filter(::entity::claim_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole claim_state state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::claim_state::Model =
                ::entity::claim_state::ModelBuilder::new(value)
                    .with_region(database_name)
                    .build();

            model
        }) {
            self.global_app_state
                .claim_state
                .insert(model.entity_id, model.clone());
            use std::collections::hash_map::Entry;
            match currently_known_claim_state.entry(model.entity_id) {
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
                insert_multiple_claim_state(
                    &self.global_app_state,
                    &on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_claim_state(&self.global_app_state, &on_conflict, &mut local_messages)
                .await;
        }

        self.queue_region_deletes(
            currently_known_claim_state.into_keys().collect(),
            database_name,
        )
        .await;
    }

    async fn handle_insert(&mut self, new: ClaimState, database_name: entity::shared::Region) {
        let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(new)
            .with_region(database_name)
            .build();

        self.global_app_state
            .claim_state
            .insert(model.entity_id, model.clone());
        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(&mut self, new: ClaimState, database_name: entity::shared::Region) {
        let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(new)
            .with_region(database_name)
            .build();

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }

        self.global_app_state
            .claim_state
            .insert(model.entity_id, model.clone());
        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(&mut self, delete: ClaimState, database_name: entity::shared::Region) {
        let model: ::entity::claim_state::Model = ::entity::claim_state::ModelBuilder::new(delete)
            .with_region(database_name)
            .build();
        let id = model.entity_id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        self.global_app_state.claim_state.remove(&id);
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

    async fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ClaimState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for ClaimStateWorker {
    type Entity = ClaimState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "claim_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages().await;
        self.flush_deletes().await;
    }
}

fn start_claim_state_db_worker(global_app_state: AppState) -> Sender<ClaimStateDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict = sea_query::OnConflict::column(claim_state::Column::EntityId)
        .update_columns([
            claim_state::Column::OwnerPlayerEntityId,
            claim_state::Column::OwnerBuildingEntityId,
            claim_state::Column::Name,
            claim_state::Column::Neutral,
            claim_state::Column::Region,
        ])
        .to_owned();

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ClaimStateDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_claim_state(&global_app_state, &on_conflict, &mut messages)
                        .await;
                }
                ClaimStateDbOperation::Delete(ids) => {
                    delete_multiple_claim_state(&global_app_state, ids).await;
                }
                ClaimStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_claim_state_for_region(&global_app_state, ids, region).await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_claim_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_state::Entity::delete_many()
            .filter(::entity::claim_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimState"
            );
        }
    }
}

async fn delete_multiple_claim_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_state::Entity::delete_many()
            .filter(::entity::claim_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::claim_state::Column::Region.eq(region.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimState"
            );
        }
    }
}

async fn insert_multiple_claim_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_state::ActiveModel>,
) {
    let insert = ::entity::claim_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ClaimState: {}", err)
    }

    messages.clear();
}

enum ClaimLocalStateDbOperation {
    Upsert(Vec<::entity::claim_local_state::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct ClaimLocalStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimLocalState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ClaimLocalState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<ClaimLocalStateDbOperation>,
    messages: Vec<::entity::claim_local_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl ClaimLocalStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_claim_local_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    fn queue_upserts(&self, messages: Vec<::entity::claim_local_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimLocalStateDbOperation::Upsert(messages))
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimLocalState upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimLocalStateDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimLocalState deletes"
            );
        }
    }

    fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimLocalStateDbOperation::DeleteForRegion { ids, region })
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimLocalState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ClaimLocalState>) {
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
        data: Vec<ClaimLocalState>,
        database_name: entity::shared::Region,
    ) {
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
                claim_local_state::Column::Region,
            ])
            .to_owned();
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_claim_local_state = ::entity::claim_local_state::Entity::find()
            .filter(::entity::claim_local_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole claim_local_state state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::claim_local_state::Model =
                ::entity::claim_local_state::ModelBuilder::new(value)
                    .with_region(database_name)
                    .build();

            model
        }) {
            let org_id = model.entity_id;
            self.global_app_state
                .claim_local_state
                .insert(org_id as u64, model.clone());
            let _ = self
                .global_app_state
                .tx
                .send(WebSocketMessages::ClaimLocalState(model.clone()));

            use std::collections::hash_map::Entry;
            match currently_known_claim_local_state.entry(model.entity_id) {
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
                insert_multiple_claim_local_state(
                    &self.global_app_state,
                    &on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_claim_local_state(
                &self.global_app_state,
                &on_conflict,
                &mut local_messages,
            )
            .await;
        }

        self.queue_region_deletes(
            currently_known_claim_local_state.into_keys().collect(),
            database_name,
        );
    }

    async fn handle_insert(&mut self, new: ClaimLocalState, database_name: entity::shared::Region) {
        let org_id = new.entity_id;
        let model: ::entity::claim_local_state::Model =
            ::entity::claim_local_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();
        self.global_app_state
            .claim_local_state
            .insert(org_id, model.clone());

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
                .position(|value: &::entity::claim_local_state::ActiveModel| {
                    value.entity_id.as_ref() == &model.entity_id
                })
        {
            self.messages.remove(index);
        }
        self.messages.push(model.clone().into_active_model());

        let _ = self
            .global_app_state
            .tx
            .send(WebSocketMessages::ClaimLocalState(model));
    }

    async fn handle_update(
        &mut self,
        new: ClaimLocalState,
        old: ClaimLocalState,
        database_name: entity::shared::Region,
    ) {
        let org_id = new.entity_id;

        if (old.treasury + 1) == new.treasury {
            metrics::counter!(
                "claim_treasury_hex_production_count",
                &[
                    ("region", database_name.to_string()),
                    ("claim_id", org_id.to_string())
                ]
            )
            .increment(1);
        }

        let model: ::entity::claim_local_state::Model =
            ::entity::claim_local_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

        self.global_app_state
            .claim_local_state
            .insert(org_id, model.clone());

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
        self.messages.push(model.clone().into_active_model());

        let _ = self
            .global_app_state
            .tx
            .send(WebSocketMessages::ClaimLocalState(model));
    }

    async fn handle_remove(
        &mut self,
        delete: ClaimLocalState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::claim_local_state::Model =
            ::entity::claim_local_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }
        self.global_app_state
            .claim_local_state
            .remove(&(model.entity_id as u64));
        self.messages_delete.push(id);
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ClaimLocalState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete);
    }
}

impl BatchedWorker for ClaimLocalStateWorker {
    type Entity = ClaimLocalState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "claim_local_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
    }
}

fn start_claim_local_state_db_worker(
    global_app_state: AppState,
) -> UnboundedSender<ClaimLocalStateDbOperation> {
    let (tx, mut rx) = unbounded_channel();
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
            claim_local_state::Column::Region,
        ])
        .to_owned();

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ClaimLocalStateDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_claim_local_state(
                        &global_app_state,
                        &on_conflict,
                        &mut messages,
                    )
                    .await;
                }
                ClaimLocalStateDbOperation::Delete(ids) => {
                    delete_multiple_claim_local_state(&global_app_state, ids).await;
                }
                ClaimLocalStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_claim_local_state_for_region(&global_app_state, ids, region)
                        .await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_claim_local_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_local_state::Entity::delete_many()
            .filter(::entity::claim_local_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimLocalState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimLocalState"
            );
        }
    }
}

async fn delete_multiple_claim_local_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_local_state::Entity::delete_many()
            .filter(::entity::claim_local_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::claim_local_state::Column::Region.eq(region.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimLocalState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimLocalState"
            );
        }
    }
}

async fn insert_multiple_claim_local_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_local_state::ActiveModel>,
) {
    let insert = ::entity::claim_local_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ClaimLocalState: {}", err)
    }

    messages.clear();
}

enum ClaimMemberStateDbOperation {
    Upsert(Vec<::entity::claim_member_state::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct ClaimMemberStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimMemberState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ClaimMemberState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<ClaimMemberStateDbOperation>,
    messages: Vec<::entity::claim_member_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl ClaimMemberStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_claim_member_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    fn queue_upserts(&self, messages: Vec<::entity::claim_member_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimMemberStateDbOperation::Upsert(messages))
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimMemberState upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimMemberStateDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimMemberState deletes"
            );
        }
    }

    fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimMemberStateDbOperation::DeleteForRegion { ids, region })
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimMemberState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ClaimMemberState>) {
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
                new, database_name, ..
            } => {
                self.handle_update(new, database_name).await;
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
        data: Vec<ClaimMemberState>,
        database_name: entity::shared::Region,
    ) {
        let on_conflict = sea_query::OnConflict::column(claim_member_state::Column::EntityId)
            .update_columns([
                claim_member_state::Column::ClaimEntityId,
                claim_member_state::Column::PlayerEntityId,
                claim_member_state::Column::UserName,
                claim_member_state::Column::InventoryPermission,
                claim_member_state::Column::BuildPermission,
                claim_member_state::Column::OfficerPermission,
                claim_member_state::Column::CoOwnerPermission,
                claim_member_state::Column::Region,
            ])
            .to_owned();
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_claim_member_state = ::entity::claim_member_state::Entity::find()
            .filter(::entity::claim_member_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole claim_member_state state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::claim_member_state::Model =
                ::entity::claim_member_state::ModelBuilder::new(value)
                    .with_region(database_name)
                    .build();

            model
        }) {
            use std::collections::hash_map::Entry;
            match currently_known_claim_member_state.entry(model.entity_id) {
                Entry::Occupied(entry) => {
                    let existing_model = entry.get();
                    if &model != existing_model {
                        local_messages.push(model.into_active_model());
                    }
                    self.global_app_state
                        .add_claim_member(existing_model.clone());
                    entry.remove();
                }
                Entry::Vacant(_entry) => {
                    local_messages.push(model.into_active_model());
                }
            }
            if local_messages.len() >= self.batch_size {
                insert_multiple_claim_member_state(
                    &self.global_app_state,
                    &on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_claim_member_state(
                &self.global_app_state,
                &on_conflict,
                &mut local_messages,
            )
            .await;
        }

        let ids = currently_known_claim_member_state
            .into_iter()
            .map(|(id, value)| {
                self.global_app_state.remove_claim_member(value.clone());
                id
            })
            .collect();
        self.queue_region_deletes(ids, database_name);
    }

    async fn handle_insert(
        &mut self,
        new: ClaimMemberState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::claim_member_state::Model =
            ::entity::claim_member_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(
        &mut self,
        new: ClaimMemberState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::claim_member_state::Model =
            ::entity::claim_member_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }

        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(
        &mut self,
        delete: ClaimMemberState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::claim_member_state::Model =
            ::entity::claim_member_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }

        self.global_app_state.remove_claim_member(model.clone());
        self.messages_delete.push(id);
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ClaimMemberState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete);
    }
}

impl BatchedWorker for ClaimMemberStateWorker {
    type Entity = ClaimMemberState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "claim_member_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
    }
}

fn start_claim_member_state_db_worker(
    global_app_state: AppState,
) -> UnboundedSender<ClaimMemberStateDbOperation> {
    let (tx, mut rx) = unbounded_channel();
    let on_conflict = sea_query::OnConflict::column(claim_member_state::Column::EntityId)
        .update_columns([
            claim_member_state::Column::ClaimEntityId,
            claim_member_state::Column::PlayerEntityId,
            claim_member_state::Column::UserName,
            claim_member_state::Column::InventoryPermission,
            claim_member_state::Column::BuildPermission,
            claim_member_state::Column::OfficerPermission,
            claim_member_state::Column::CoOwnerPermission,
            claim_member_state::Column::Region,
        ])
        .to_owned();

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ClaimMemberStateDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_claim_member_state(
                        &global_app_state,
                        &on_conflict,
                        &mut messages,
                    )
                    .await;
                }
                ClaimMemberStateDbOperation::Delete(ids) => {
                    delete_multiple_claim_member_state(&global_app_state, ids).await;
                }
                ClaimMemberStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_claim_member_state_for_region(&global_app_state, ids, region)
                        .await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_claim_member_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_member_state::Entity::delete_many()
            .filter(::entity::claim_member_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimMemberState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimMemberState"
            );
        }
    }
}

async fn delete_multiple_claim_member_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_member_state::Entity::delete_many()
            .filter(::entity::claim_member_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::claim_member_state::Column::Region.eq(region.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimMemberState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimMemberState"
            );
        }
    }
}

async fn insert_multiple_claim_member_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_member_state::ActiveModel>,
) {
    let insert = ::entity::claim_member_state::Entity::insert_many(
        messages
            .iter()
            .map(|value| {
                global_app_state.add_claim_member(value.clone().try_into_model().unwrap());
                value.clone()
            })
            .collect::<Vec<_>>(),
    )
    .on_conflict(on_conflict.clone())
    .exec(&global_app_state.conn)
    .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ClaimMemberState: {}", err)
    }

    messages.clear();
}

enum ClaimTechStateDbOperation {
    Upsert(Vec<::entity::claim_tech_state::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct ClaimTechStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ClaimTechState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<ClaimTechStateDbOperation>,
    messages: Vec<::entity::claim_tech_state::ActiveModel>,
    messages_delete: Vec<i64>,
}

impl ClaimTechStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_claim_tech_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    fn queue_upserts(&self, messages: Vec<::entity::claim_tech_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimTechStateDbOperation::Upsert(messages)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimTechState upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimTechStateDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimTechState deletes"
            );
        }
    }

    fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ClaimTechStateDbOperation::DeleteForRegion { ids, region })
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimTechState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ClaimTechState>) {
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
                new, database_name, ..
            } => {
                self.handle_update(new, database_name).await;
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
        data: Vec<ClaimTechState>,
        database_name: entity::shared::Region,
    ) {
        let on_conflict = sea_query::OnConflict::columns([claim_tech_state::Column::EntityId])
            .update_columns([
                claim_tech_state::Column::Learned,
                claim_tech_state::Column::Researching,
                claim_tech_state::Column::StartTimestamp,
                claim_tech_state::Column::ScheduledId,
                claim_tech_state::Column::Region,
            ])
            .to_owned();
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_claim_tech_state = ::entity::claim_tech_state::Entity::find()
            .filter(::entity::claim_tech_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole claim_tech_state state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::claim_tech_state::Model =
                ::entity::claim_tech_state::ModelBuilder::new(value)
                    .with_region(database_name)
                    .build();

            model
        }) {
            use std::collections::hash_map::Entry;
            match currently_known_claim_tech_state.entry(model.entity_id) {
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
                insert_multiple_claim_tech_state(
                    &self.global_app_state,
                    &on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_claim_tech_state(
                &self.global_app_state,
                &on_conflict,
                &mut local_messages,
            )
            .await;
        }

        self.queue_region_deletes(
            currently_known_claim_tech_state.into_keys().collect(),
            database_name,
        );
    }

    async fn handle_insert(&mut self, new: ClaimTechState, database_name: entity::shared::Region) {
        let model: ::entity::claim_tech_state::Model =
            ::entity::claim_tech_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

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
                .position(|value: &::entity::claim_tech_state::ActiveModel| {
                    value.entity_id.as_ref() == &model.entity_id
                })
        {
            self.messages.remove(index);
        }

        self.messages.push(model.into_active_model());
    }

    async fn handle_update(&mut self, new: ClaimTechState, database_name: entity::shared::Region) {
        let model: ::entity::claim_tech_state::Model =
            ::entity::claim_tech_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();

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
                .position(|value: &::entity::claim_tech_state::ActiveModel| {
                    value.entity_id.as_ref() == &model.entity_id
                })
        {
            self.messages.remove(index);
        }

        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(
        &mut self,
        delete: ClaimTechState,
        database_name: entity::shared::Region,
    ) {
        let model: ::entity::claim_tech_state::Model =
            ::entity::claim_tech_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            self.messages.remove(index);
        }
        self.messages_delete.push(id);
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ClaimTechState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete);
    }
}

impl BatchedWorker for ClaimTechStateWorker {
    type Entity = ClaimTechState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "claim_tech_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
    }
}

fn start_claim_tech_state_db_worker(
    global_app_state: AppState,
) -> UnboundedSender<ClaimTechStateDbOperation> {
    let (tx, mut rx) = unbounded_channel();
    let on_conflict = sea_query::OnConflict::columns([claim_tech_state::Column::EntityId])
        .update_columns([
            claim_tech_state::Column::Learned,
            claim_tech_state::Column::Researching,
            claim_tech_state::Column::StartTimestamp,
            claim_tech_state::Column::ScheduledId,
            claim_tech_state::Column::Region,
        ])
        .to_owned();

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ClaimTechStateDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_claim_tech_state(
                        &global_app_state,
                        &on_conflict,
                        &mut messages,
                    )
                    .await;
                }
                ClaimTechStateDbOperation::Delete(ids) => {
                    delete_multiple_claim_tech_state(&global_app_state, ids).await;
                }
                ClaimTechStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_claim_tech_state_for_region(&global_app_state, ids, region)
                        .await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_claim_tech_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_tech_state::Entity::delete_many()
            .filter(::entity::claim_tech_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimTechState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimTechState"
            );
        }
    }
}

async fn delete_multiple_claim_tech_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_tech_state::Entity::delete_many()
            .filter(::entity::claim_tech_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::claim_tech_state::Column::Region.eq(region.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimTechState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimTechState"
            );
        }
    }
}

async fn insert_multiple_claim_tech_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_tech_state::ActiveModel>,
) {
    let insert = ::entity::claim_tech_state::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ClaimTechState: {}", err)
    }

    messages.clear();
}

enum ClaimTechDescDbOperation {
    Upsert(Vec<::entity::claim_tech_desc::ActiveModel>),
    Delete(Vec<i32>),
}

pub(crate) struct ClaimTechDescWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ClaimTechDesc>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ClaimTechDesc>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: UnboundedSender<ClaimTechDescDbOperation>,
    messages: Vec<::entity::claim_tech_desc::ActiveModel>,
    messages_delete: Vec<i32>,
}

impl ClaimTechDescWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_claim_tech_desc_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
        }
    }

    fn queue_upserts(&self, messages: Vec<::entity::claim_tech_desc::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimTechDescDbOperation::Upsert(messages)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimTechDesc upserts"
            );
        }
    }

    fn queue_deletes(&self, ids: Vec<i32>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(ClaimTechDescDbOperation::Delete(ids)) {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ClaimTechDesc deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ClaimTechDesc>) {
        match msg {
            SpacetimeUpdateMessages::Initial { data, .. } => {
                self.handle_initial(data).await;
            }
            SpacetimeUpdateMessages::Insert { new, .. } => {
                self.handle_insert(new).await;
            }
            SpacetimeUpdateMessages::Update { new, .. } => {
                self.handle_update(new).await;
            }
            SpacetimeUpdateMessages::Remove { delete, .. } => {
                self.handle_remove(delete).await;
            }
        }
    }

    async fn handle_initial(&mut self, data: Vec<ClaimTechDesc>) {
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
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_claim_tech_desc = ::entity::claim_tech_desc::Entity::find()
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole claim_tech_desc state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.id, value))
            .collect::<HashMap<_, _>>();

        for model in data.into_iter().map(|value| {
            let model: ::entity::claim_tech_desc::Model = value.into();

            model
        }) {
            use std::collections::hash_map::Entry;
            match currently_known_claim_tech_desc.entry(model.id) {
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
                insert_multiple_claim_tech_desc(
                    &self.global_app_state,
                    &on_conflict,
                    &mut local_messages,
                )
                .await;
            }
        }
        if !local_messages.is_empty() {
            insert_multiple_claim_tech_desc(
                &self.global_app_state,
                &on_conflict,
                &mut local_messages,
            )
            .await;
        }

        self.queue_deletes(currently_known_claim_tech_desc.into_keys().collect());
    }

    async fn handle_insert(&mut self, new: ClaimTechDesc) {
        let model: ::entity::claim_tech_desc::Model = new.into();

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(&mut self, new: ClaimTechDesc) {
        let model: ::entity::claim_tech_desc::Model = new.into();
        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.id)
        {
            self.messages_delete.remove(index);
        }
        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(&mut self, delete: ClaimTechDesc) {
        let model: ::entity::claim_tech_desc::Model = delete.into();
        let id = model.id;

        if let Some(index) = self
            .messages
            .iter()
            .position(|value| value.id.as_ref() == &model.id)
        {
            self.messages.remove(index);
        }
        self.messages_delete.push(id);
    }

    fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages =
            std::mem::replace(&mut self.messages, Vec::with_capacity(self.batch_size + 10));
        self.queue_upserts(messages);
    }

    fn flush_deletes(&mut self) {
        if self.messages_delete.is_empty() {
            return;
        }

        tracing::debug!("ClaimTechDesc::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete);
    }
}

impl BatchedWorker for ClaimTechDescWorker {
    type Entity = ClaimTechDesc;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "claim_tech_desc"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages();
        self.flush_deletes();
    }
}

fn start_claim_tech_desc_db_worker(
    global_app_state: AppState,
) -> UnboundedSender<ClaimTechDescDbOperation> {
    let (tx, mut rx) = unbounded_channel();
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

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ClaimTechDescDbOperation::Upsert(messages) => {
                    let mut messages = messages;
                    insert_multiple_claim_tech_desc(&global_app_state, &on_conflict, &mut messages)
                        .await;
                }
                ClaimTechDescDbOperation::Delete(ids) => {
                    delete_multiple_claim_tech_desc(&global_app_state, ids).await;
                }
            }
        }
    });

    tx
}

async fn delete_multiple_claim_tech_desc(global_app_state: &AppState, ids: Vec<i32>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::claim_tech_desc::Entity::delete_many()
            .filter(::entity::claim_tech_desc::Column::Id.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                ClaimTechDesc = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete ClaimTechDesc"
            );
        }
    }
}

async fn insert_multiple_claim_tech_desc(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: &mut Vec<::entity::claim_tech_desc::ActiveModel>,
) {
    let insert = ::entity::claim_tech_desc::Entity::insert_many(messages.clone())
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting ClaimTechDesc: {}", err)
    }

    messages.clear();
}
