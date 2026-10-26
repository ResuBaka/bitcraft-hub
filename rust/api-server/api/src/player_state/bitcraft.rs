use crate::AppState;
use crate::websocket::batched_worker::{BatchTimer, BatchedWorker};
use crate::websocket::worker_channel::{UnboundedReceiver, UnboundedSender};
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages};
use game_module::module_bindings::{PlayerState, PlayerUsernameState};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{Sender, channel};
use tokio::time::Instant;

const PLAYER_DELETE_GRACE: Duration = Duration::from_secs(30);

struct PendingPlayerDelete {
    region: entity::shared::Region,
    deadline: Instant,
}

enum PlayerStateDbOperation {
    Upsert(Vec<::entity::player_state::ActiveModel>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

enum PlayerUsernameStateDbOperation {
    Upsert(Vec<::entity::player_username_state::ActiveModel>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

pub(crate) struct PlayerStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<PlayerState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<PlayerStateDbOperation>,
    messages: Vec<::entity::player_state::ActiveModel>,
    ids: Vec<i64>,
    pending_deletes: HashMap<i64, PendingPlayerDelete>,
}

impl PlayerStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_player_state_db_worker(global_app_state.clone());
        let (tx, rx) =
            crate::websocket::worker_channel::shutdown_channel(&global_app_state.shutdown);

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::new(),
            ids: Vec::new(),
            pending_deletes: HashMap::new(),
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::player_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(PlayerStateDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerState upserts"
            );
        }
    }

    async fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(PlayerStateDbOperation::DeleteForRegion { ids, region })
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<PlayerState>) {
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
                database_name,
                old,
                ..
            } => {
                self.handle_update(new, database_name, old).await;
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
        data: Vec<PlayerState>,
        database_name: entity::shared::Region,
    ) {
        // Older live writes must precede this newer snapshot in the DB queue.
        self.flush_messages().await;
        self.ids.clear();
        let mut local_messages = Vec::with_capacity(self.batch_size + 10);
        let mut currently_known_player_state = ::entity::player_state::Entity::find()
            .filter(::entity::player_state::Column::Region.eq(database_name))
            .all(&self.global_app_state.conn)
            .await
            .unwrap_or_else(|error| {
                tracing::error!(
                    error = error.to_string(),
                    "Error while query whole player_state state"
                );
                vec![]
            })
            .into_iter()
            .map(|value| (value.entity_id, value))
            .collect::<HashMap<_, _>>();

        let mut online = 0;
        let mut offline = 0;

        for mut model in data.into_iter().map(|value| {
            let model: ::entity::player_state::Model =
                ::entity::player_state::ModelBuilder::new(value)
                    .with_region(database_name)
                    .build();

            if model.signed_in {
                online += 1;
            } else {
                offline += 1;
            }

            model
        }) {
            self.pending_deletes.remove(&model.entity_id);
            use std::collections::hash_map::Entry;
            match currently_known_player_state.entry(model.entity_id) {
                Entry::Occupied(entry) => {
                    let existing_model = entry.get();
                    if &model != existing_model {
                        if model.sign_in_timestamp == 0 {
                            model.sign_in_timestamp = existing_model.sign_in_timestamp;
                        }
                        self.global_app_state
                            .player_state
                            .insert(model.entity_id, model.clone());
                        self.global_app_state
                            .ranking_system
                            .time_played
                            .update(model.entity_id, model.time_played as i64);
                        self.global_app_state
                            .ranking_system
                            .time_signed_in
                            .update(model.entity_id, model.time_signed_in as i64);
                        local_messages.push(model.into_active_model());
                    }
                    entry.remove();
                }
                Entry::Vacant(_entry) => {
                    self.global_app_state
                        .player_state
                        .insert(model.entity_id, model.clone());
                    self.global_app_state
                        .ranking_system
                        .time_played
                        .update(model.entity_id, model.time_played as i64);
                    self.global_app_state
                        .ranking_system
                        .time_signed_in
                        .update(model.entity_id, model.time_signed_in as i64);
                    local_messages.push(model.into_active_model());
                }
            }
            if local_messages.len() >= self.batch_size {
                self.queue_upserts(std::mem::take(&mut local_messages))
                    .await;
            }
        }

        if !local_messages.is_empty() {
            self.queue_upserts(local_messages).await;
        }

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", "false".to_string()),
                ("region", database_name.to_string())
            ]
        )
        .set(offline);

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", "true".to_string()),
                ("region", database_name.to_string())
            ]
        )
        .set(online);

        for id in currently_known_player_state.into_keys() {
            self.schedule_delete(id, database_name);
        }
    }

    async fn handle_insert(&mut self, new: PlayerState, database_name: entity::shared::Region) {
        let mut model: ::entity::player_state::Model =
            ::entity::player_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();
        self.global_app_state
            .ranking_system
            .time_played
            .update(model.entity_id, model.time_played as i64);
        self.global_app_state
            .ranking_system
            .time_signed_in
            .update(model.entity_id, model.time_signed_in as i64);

        self.pending_deletes.remove(&model.entity_id);

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", model.signed_in.to_string()),
                ("region", database_name.to_string())
            ]
        )
        .increment(1);

        if self.ids.contains(&model.entity_id)
            && let Some(index) =
                self.messages
                    .iter()
                    .position(|value: &::entity::player_state::ActiveModel| {
                        value.entity_id.as_ref() == &model.entity_id
                    })
        {
            if model.sign_in_timestamp == 0 {
                model.sign_in_timestamp = self.messages[index].sign_in_timestamp.clone().unwrap();
            }
            self.messages.remove(index);
        }

        self.global_app_state
            .player_state
            .insert(model.entity_id, model.clone());
        self.ids.push(model.entity_id);
        self.messages.push(model.into_active_model());
    }

    async fn handle_update(
        &mut self,
        new: PlayerState,
        database_name: entity::shared::Region,
        old: PlayerState,
    ) {
        let mut model: ::entity::player_state::Model =
            ::entity::player_state::ModelBuilder::new(new)
                .with_region(database_name)
                .build();
        self.global_app_state
            .ranking_system
            .time_played
            .update(model.entity_id, model.time_played as i64);
        let rank = self
            .global_app_state
            .ranking_system
            .time_played
            .get_rank(model.time_played as i64);
        if let Some(rank) = rank {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::TimePlayed {
                    user_id: model.entity_id,
                    time: model.time_played as u64,
                    rank: rank as u64,
                });
        }

        self.global_app_state
            .ranking_system
            .time_signed_in
            .update(model.entity_id, model.time_signed_in as i64);
        let rank = self
            .global_app_state
            .ranking_system
            .time_signed_in
            .get_rank(model.time_played as i64);
        if let Some(rank) = rank {
            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::TimeSignedIn {
                    user_id: model.entity_id,
                    time: model.time_played as u64,
                    rank: rank as u64,
                });
        }

        if model.signed_in != old.signed_in {
            metrics::gauge!(
                "players_current_state",
                &[
                    ("online", model.signed_in.to_string()),
                    ("region", database_name.to_string())
                ]
            )
            .increment(1);
            metrics::gauge!(
                "players_current_state",
                &[
                    ("online", old.signed_in.to_string()),
                    ("region", database_name.to_string())
                ]
            )
            .decrement(1);
        }

        if self.ids.contains(&model.entity_id)
            && let Some(index) = self
                .messages
                .iter()
                .position(|value| value.entity_id.as_ref() == &model.entity_id)
        {
            if model.sign_in_timestamp == 0 {
                model.sign_in_timestamp = self.messages[index].sign_in_timestamp.clone().unwrap();
            }
            self.messages.remove(index);
        }

        self.pending_deletes.remove(&model.entity_id);

        if model.sign_in_timestamp == 0 {
            model.sign_in_timestamp = old.sign_in_timestamp;
        }

        self.ids.push(model.entity_id);

        self.global_app_state
            .player_state
            .insert(model.entity_id, model.clone());
        let _ = self
            .global_app_state
            .tx
            .send(OutboundWebSocketMessages::PlayerState(model.clone()));
        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(&mut self, delete: PlayerState, database_name: entity::shared::Region) {
        let model: ::entity::player_state::Model =
            ::entity::player_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", model.signed_in.to_string()),
                ("region", database_name.to_string())
            ]
        )
        .decrement(1);

        // Do not discard pending upserts: this may be the source half of a
        // transfer whose destination has already arrived in the same batch.
        self.schedule_delete(id, database_name);
    }

    fn belongs_to_other_region(&self, id: i64, region: entity::shared::Region) -> bool {
        self.global_app_state
            .player_state
            .get(&id)
            .is_some_and(|player| player.region != region)
    }

    fn schedule_delete(&mut self, id: i64, region: entity::shared::Region) {
        if self.belongs_to_other_region(id, region) {
            return;
        }
        // Duplicate removes must not extend the original grace period.
        self.pending_deletes
            .entry(id)
            .or_insert(PendingPlayerDelete {
                region,
                deadline: Instant::now() + PLAYER_DELETE_GRACE,
            });
    }

    async fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages);
        self.queue_upserts(messages).await;
    }

    async fn flush_deletes(&mut self) {
        let now = Instant::now();
        let mut deletes_by_region = HashMap::<_, Vec<_>>::new();
        let due = self
            .pending_deletes
            .iter()
            .filter(|(_, delete)| delete.deadline <= now)
            .map(|(&id, delete)| (id, delete.region))
            .collect::<Vec<_>>();
        for (id, region) in due {
            self.pending_deletes.remove(&id);
            if !self.belongs_to_other_region(id, region) {
                deletes_by_region.entry(region).or_default().push(id);
            }
        }
        for (region, ids) in deletes_by_region {
            self.queue_region_deletes(ids, region).await;
        }
    }

    async fn run(mut self) {
        let receive_metrics = crate::websocket::WorkerReceiveMetrics::new("player_state");
        let mut timer = BatchTimer::new(self.time_limit);
        let mut input_closed = false;
        tracing::info!(worker = "player_state", "Worker started");
        loop {
            let delete_deadline = self
                .pending_deletes
                .values()
                .map(|delete| delete.deadline)
                .min();
            tokio::select! {
                msg = self.rx.recv(), if !input_closed => {
                    if let Some(msg) = msg {
                        timer.start();
                        receive_metrics.record(1);
                        self.process_message(msg).await;
                        if !self.should_flush() { continue; }
                    } else {
                        input_closed = true;
                    }
                    self.flush_messages().await;
                    self.ids.clear();
                    timer = BatchTimer::new(self.time_limit);
                }
                _ = timer.elapsed() => {
                    self.flush_messages().await;
                    self.ids.clear();
                    timer = BatchTimer::new(self.time_limit);
                }
                _ = async {
                    match delete_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                } => {
                    // Serialize preceding upserts before any confirmed deletion.
                    self.flush_messages().await;
                    self.ids.clear();
                    self.flush_deletes().await;
                }
            }
            // Drain on shutdown without shortening the transfer grace period.
            if input_closed && self.is_idle() {
                break;
            }
        }
    }
}

impl BatchedWorker for PlayerStateWorker {
    type Entity = PlayerState;

    fn start(self, shutdown: &crate::shutdown::Shutdown) {
        shutdown.spawn_worker(self.run());
    }

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "player_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.pending_deletes.is_empty()
    }

    fn reset_batch(&mut self) {
        self.ids.clear();
    }

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<Self::Entity>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages().await;
        self.flush_deletes().await;
    }
}

async fn insert_multiple_player_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: Vec<::entity::player_state::ActiveModel>,
) {
    let insert = ::entity::player_state::Entity::insert_many(messages)
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting PlayerState: {}", err)
    }
}

async fn delete_multiple_player_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::player_state::Entity::delete_many()
            .filter(::entity::player_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::player_state::Column::Region.eq(region))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                PlayerState = chunk_ids_str.join(","),
                region,
                error = error.to_string(),
                "Could not delete PlayerState"
            );
        }
    }
}

fn start_player_state_db_worker(global_app_state: AppState) -> Sender<PlayerStateDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict = sea_query::OnConflict::column(::entity::player_state::Column::EntityId)
        .update_columns([
            ::entity::player_state::Column::TimePlayed,
            ::entity::player_state::Column::SessionStartTimestamp,
            ::entity::player_state::Column::TimeSignedIn,
            ::entity::player_state::Column::SignInTimestamp,
            ::entity::player_state::Column::SignedIn,
            ::entity::player_state::Column::TeleportLocation,
            ::entity::player_state::Column::TravelerTasksExpiration,
            ::entity::player_state::Column::Region,
        ])
        .to_owned();

    global_app_state.shutdown.clone().spawn_worker(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                PlayerStateDbOperation::Upsert(messages) => {
                    insert_multiple_player_state(&global_app_state, &on_conflict, messages).await;
                }
                PlayerStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_player_state_for_region(&global_app_state, ids, region).await;
                }
            }
        }
    });

    tx
}

pub(crate) struct PlayerUsernameStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<PlayerUsernameState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<PlayerUsernameState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<PlayerUsernameStateDbOperation>,
    messages: Vec<::entity::player_username_state::ActiveModel>,
    pending_deletes: HashMap<i64, PendingPlayerDelete>,
    // Username rows have no AppState cache. Keep only their latest observed
    // region so a late source-region remove cannot supersede a destination row.
    regions: HashMap<i64, entity::shared::Region>,
}

impl PlayerUsernameStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_player_username_state_db_worker(global_app_state.clone());
        let (tx, rx) =
            crate::websocket::worker_channel::shutdown_channel(&global_app_state.shutdown);

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::new(),
            pending_deletes: HashMap::new(),
            regions: HashMap::new(),
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::player_username_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(PlayerUsernameStateDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerUsernameState upserts"
            );
        }
    }

    async fn queue_region_deletes(&self, ids: Vec<i64>, region: entity::shared::Region) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(PlayerUsernameStateDbOperation::DeleteForRegion { ids, region })
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerUsernameState region deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<PlayerUsernameState>) {
        match msg {
            SpacetimeUpdateMessages::Initial {
                data,
                database_name,
                ..
            } => {
                self.flush_messages().await;
                let mut local_messages = Vec::with_capacity(self.batch_size + 10);
                let mut currently_known_player_username_state =
                    ::entity::player_username_state::Entity::find()
                        .filter(::entity::player_username_state::Column::Region.eq(database_name))
                        .all(&self.global_app_state.conn)
                        .await
                        .unwrap_or_else(|error| {
                            tracing::error!(
                                error = error.to_string(),
                                "Error while query whole player_username_state state"
                            );
                            vec![]
                        })
                        .into_iter()
                        .map(|value| (value.entity_id, value))
                        .collect::<HashMap<_, _>>();

                for model in data.into_iter().map(|value| {
                    let model: ::entity::player_username_state::Model =
                        ::entity::player_username_state::ModelBuilder::new(value)
                            .with_region(database_name)
                            .build();

                    model
                }) {
                    // Cancel even for unchanged rows that will not be upserted.
                    self.pending_deletes.remove(&model.entity_id);
                    self.regions.insert(model.entity_id, database_name);
                    use std::collections::hash_map::Entry;
                    match currently_known_player_username_state.entry(model.entity_id) {
                        Entry::Occupied(entry) => {
                            let existing_model = entry.get();
                            if &model != existing_model {
                                let _ = self.global_app_state.tx.send(
                                    OutboundWebSocketMessages::PlayerUsername {
                                        entity_id: model.entity_id,
                                        username: model.username.clone(),
                                    },
                                );
                                local_messages.push(model.into_active_model());
                            }
                            entry.remove();
                        }
                        Entry::Vacant(_entry) => {
                            local_messages.push(model.into_active_model());
                        }
                    }
                    if local_messages.len() >= self.batch_size {
                        self.queue_upserts(std::mem::take(&mut local_messages))
                            .await;
                    }
                }
                if !local_messages.is_empty() {
                    self.queue_upserts(local_messages).await;
                }

                for id in currently_known_player_username_state.into_keys() {
                    self.schedule_delete(id, database_name);
                }
            }
            SpacetimeUpdateMessages::Insert {
                new, database_name, ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(new)
                        .with_region(database_name)
                        .build();

                self.pending_deletes.remove(&model.entity_id);
                self.regions.insert(model.entity_id, database_name);
                self.messages
                    .retain(|row| *row.entity_id.as_ref() != model.entity_id);
                let _ = self
                    .global_app_state
                    .tx
                    .send(OutboundWebSocketMessages::PlayerUsername {
                        entity_id: model.entity_id,
                        username: model.username.clone(),
                    });
                self.messages.push(model.into_active_model());
            }
            SpacetimeUpdateMessages::Update {
                new, database_name, ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(new)
                        .with_region(database_name)
                        .build();
                self.pending_deletes.remove(&model.entity_id);
                self.regions.insert(model.entity_id, database_name);
                self.messages
                    .retain(|row| *row.entity_id.as_ref() != model.entity_id);
                let _ = self
                    .global_app_state
                    .tx
                    .send(OutboundWebSocketMessages::PlayerUsername {
                        entity_id: model.entity_id,
                        username: model.username.clone(),
                    });
                self.messages.push(model.into_active_model());
            }
            SpacetimeUpdateMessages::Remove {
                delete,
                database_name,
                ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(delete)
                        .with_region(database_name)
                        .build();
                self.schedule_delete(model.entity_id, database_name);
            }
        }
    }

    async fn flush_messages(&mut self) {
        if self.messages.is_empty() {
            return;
        }

        let messages = std::mem::take(&mut self.messages);
        self.queue_upserts(messages).await;
    }

    fn schedule_delete(&mut self, id: i64, region: entity::shared::Region) {
        if self
            .regions
            .get(&id)
            .is_some_and(|current| *current != region)
        {
            return;
        }
        self.pending_deletes
            .entry(id)
            .or_insert(PendingPlayerDelete {
                region,
                deadline: Instant::now() + PLAYER_DELETE_GRACE,
            });
    }

    async fn flush_deletes(&mut self) {
        let now = Instant::now();
        let due = self
            .pending_deletes
            .iter()
            .filter(|(_, delete)| delete.deadline <= now)
            .map(|(&id, delete)| (id, delete.region))
            .collect::<Vec<_>>();
        let mut deletes_by_region = HashMap::<_, Vec<_>>::new();
        for (id, region) in due {
            self.pending_deletes.remove(&id);
            if self
                .regions
                .get(&id)
                .is_none_or(|current| *current == region)
            {
                self.regions.remove(&id);
                deletes_by_region.entry(region).or_default().push(id);
            }
        }
        for (region, ids) in deletes_by_region {
            self.queue_region_deletes(ids, region).await;
        }
    }

    async fn run(mut self) {
        let receive_metrics = crate::websocket::WorkerReceiveMetrics::new("player_username_state");
        let mut timer = BatchTimer::new(self.time_limit);
        let mut input_closed = false;
        tracing::info!(worker = "player_username_state", "Worker started");
        loop {
            let delete_deadline = self
                .pending_deletes
                .values()
                .map(|delete| delete.deadline)
                .min();
            tokio::select! {
                msg = self.rx.recv(), if !input_closed => {
                    if let Some(msg) = msg {
                        timer.start();
                        receive_metrics.record(1);
                        self.process_message(msg).await;
                        if !self.should_flush() { continue; }
                    } else {
                        input_closed = true;
                    }
                    self.flush_messages().await;
                    timer = BatchTimer::new(self.time_limit);
                }
                _ = timer.elapsed() => {
                    self.flush_messages().await;
                    timer = BatchTimer::new(self.time_limit);
                }
                _ = async {
                    match delete_deadline {
                        Some(deadline) => tokio::time::sleep_until(deadline).await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.flush_messages().await;
                    self.flush_deletes().await;
                }
            }
            if input_closed && self.is_idle() {
                break;
            }
        }
    }
}

impl BatchedWorker for PlayerUsernameStateWorker {
    type Entity = PlayerUsernameState;

    fn start(self, shutdown: &crate::shutdown::Shutdown) {
        shutdown.spawn_worker(self.run());
    }
    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "player_username_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.messages.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.pending_deletes.is_empty()
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

async fn insert_multiple_player_username_state(
    global_app_state: &AppState,
    on_conflict: &sea_query::OnConflict,
    messages: Vec<::entity::player_username_state::ActiveModel>,
) {
    let insert = ::entity::player_username_state::Entity::insert_many(messages)
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(err) = insert {
        tracing::error!("Error inserting PlayerUsernameState: {}", err)
    }
}

async fn delete_multiple_player_username_state_for_region(
    global_app_state: &AppState,
    ids: Vec<i64>,
    region: entity::shared::Region,
) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::player_username_state::Entity::delete_many()
            .filter(::entity::player_username_state::Column::EntityId.is_in(chunk_ids.clone()))
            .filter(::entity::player_username_state::Column::Region.eq(region))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                PlayerUsernameState = chunk_ids_str.join(","),
                region,
                error = error.to_string(),
                "Could not delete PlayerUsernameState"
            );
        }
    }
}

fn start_player_username_state_db_worker(
    global_app_state: AppState,
) -> Sender<PlayerUsernameStateDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict =
        sea_query::OnConflict::column(::entity::player_username_state::Column::EntityId)
            .update_columns([
                ::entity::player_username_state::Column::Username,
                ::entity::player_username_state::Column::Region,
            ])
            .to_owned();

    global_app_state.shutdown.clone().spawn_worker(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                PlayerUsernameStateDbOperation::Upsert(messages) => {
                    insert_multiple_player_username_state(
                        &global_app_state,
                        &on_conflict,
                        messages,
                    )
                    .await;
                }
                PlayerUsernameStateDbOperation::DeleteForRegion { ids, region } => {
                    delete_multiple_player_username_state_for_region(
                        &global_app_state,
                        ids,
                        region,
                    )
                    .await;
                }
            }
        }
    });

    tx
}

#[cfg(test)]
mod username_tests {
    use super::*;
    use sea_orm::{DbBackend, MockDatabase, MockExecResult, Transaction};

    fn username(id: u64, name: &str) -> PlayerUsernameState {
        PlayerUsernameState {
            entity_id: id,
            username: name.into(),
        }
    }

    fn model(
        id: u64,
        name: &str,
        region: entity::shared::Region,
    ) -> ::entity::player_username_state::Model {
        ::entity::player_username_state::ModelBuilder::new(username(id, name))
            .with_region(region)
            .build()
    }

    fn worker(
        db: sea_orm::DatabaseConnection,
    ) -> (
        PlayerUsernameStateWorker,
        tokio::sync::mpsc::Receiver<PlayerUsernameStateDbOperation>,
    ) {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, tx, Default::default());
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        let (db_tx, db_rx) = channel(20);
        (
            PlayerUsernameStateWorker {
                rx,
                tx,
                global_app_state: state,
                db_tx,
                batch_size: 10,
                time_limit: Duration::from_millis(10),
                messages: Vec::new(),
                pending_deletes: HashMap::new(),
                regions: HashMap::new(),
            },
            db_rx,
        )
    }

    fn remove(
        id: u64,
        region: entity::shared::Region,
    ) -> SpacetimeUpdateMessages<PlayerUsernameState> {
        SpacetimeUpdateMessages::Remove {
            event: None,
            delete: username(id, "name"),
            database_name: region,
        }
    }

    fn initial(
        data: Vec<PlayerUsernameState>,
        region: entity::shared::Region,
    ) -> SpacetimeUpdateMessages<PlayerUsernameState> {
        SpacetimeUpdateMessages::Initial {
            data,
            database_name: region,
            batches: None,
        }
    }

    #[tokio::test]
    async fn initial_rows_cancel_deletes_even_when_the_database_row_is_unchanged() {
        for existing in [
            Vec::new(),
            vec![model(1, "old", 2)],
            vec![model(1, "name", 2)],
        ] {
            let unchanged = existing.first().is_some_and(|row| row.username == "name");
            let db = MockDatabase::new(DbBackend::Postgres)
                .append_query_results([existing])
                .into_connection();
            let (mut worker, mut operations) = worker(db);
            worker.process_message(remove(1, 1)).await;
            assert!(worker.pending_deletes.contains_key(&1));
            worker
                .process_message(initial(vec![username(1, "name")], 2))
                .await;
            assert!(worker.pending_deletes.is_empty());
            assert_eq!(worker.regions[&1], 2);
            worker.flush().await;
            if !unchanged {
                assert!(
                    matches!(operations.try_recv().unwrap(), PlayerUsernameStateDbOperation::Upsert(rows) if rows.len() == 1)
                );
            }
            assert!(operations.try_recv().is_err());
            worker.process_message(remove(1, 1)).await;
            assert!(
                worker.pending_deletes.is_empty(),
                "late source remove must be ignored"
            );
        }
    }

    #[tokio::test]
    async fn missing_snapshot_rows_are_delayed_and_region_scoped() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![model(1, "name", 2)]])
            .into_connection();
        let (mut worker, mut operations) = worker(db);
        let before = Instant::now();
        worker.process_message(initial(Vec::new(), 2)).await;
        assert!(worker.pending_deletes[&1].deadline >= before + PLAYER_DELETE_GRACE);
        worker.flush().await;
        assert!(operations.try_recv().is_err());
        worker.pending_deletes.get_mut(&1).unwrap().deadline = Instant::now();
        worker.flush().await;
        assert!(matches!(operations.try_recv().unwrap(),
            PlayerUsernameStateDbOperation::DeleteForRegion { ids, region: 2 } if ids == [1]));
        assert!(worker.is_idle());
    }

    #[tokio::test]
    async fn inserts_and_updates_cancel_deletes_and_keep_latest_destination_row() {
        for update in [false, true] {
            let (mut worker, mut operations) =
                worker(MockDatabase::new(DbBackend::Postgres).into_connection());
            worker.process_message(remove(1, 1)).await;
            let deadline = worker.pending_deletes[&1].deadline;
            worker.process_message(remove(1, 1)).await;
            assert_eq!(worker.pending_deletes[&1].deadline, deadline);
            worker.flush().await;
            assert!(operations.try_recv().is_err());
            worker
                .process_message(SpacetimeUpdateMessages::Insert {
                    event: None,
                    new: username(1, "source"),
                    database_name: 1,
                })
                .await;
            let destination = if update {
                SpacetimeUpdateMessages::Update {
                    event: None,
                    new: username(1, "destination"),
                    old: username(1, "source"),
                    database_name: 2,
                }
            } else {
                SpacetimeUpdateMessages::Insert {
                    event: None,
                    new: username(1, "destination"),
                    database_name: 2,
                }
            };
            // Re-schedule the source removal so destination arrival cancels it.
            worker.process_message(remove(1, 1)).await;
            worker.process_message(destination).await;
            worker.process_message(remove(1, 1)).await;
            assert!(worker.pending_deletes.is_empty());
            worker.flush().await;
            assert!(matches!(operations.try_recv().unwrap(),
                PlayerUsernameStateDbOperation::Upsert(rows) if rows == [model(1, "destination", 2).into_active_model()]));
            assert!(operations.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn username_runner_preserves_delete_grace_while_draining_shutdown() {
        let (worker, mut operations) =
            worker(MockDatabase::new(DbBackend::Postgres).into_connection());
        worker.tx().send(remove(1, 1)).unwrap();
        let shutdown = worker.global_app_state.shutdown.clone();
        let before = Instant::now();
        worker.start(&shutdown);
        let task = tokio::spawn(async move {
            shutdown.drain().await;
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), operations.recv())
                .await
                .is_err()
        );
        let operation = tokio::time::timeout(
            PLAYER_DELETE_GRACE + Duration::from_secs(2),
            operations.recv(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(before.elapsed() >= PLAYER_DELETE_GRACE);
        assert!(matches!(operation,
            PlayerUsernameStateDbOperation::DeleteForRegion { ids, region: 1 } if ids == [1]));
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn username_delete_sql_includes_source_region() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let (worker, _) = worker(db);
        delete_multiple_player_username_state_for_region(&worker.global_app_state, vec![1], 2)
            .await;
        assert_eq!(
            worker.global_app_state.conn.into_transaction_log(),
            [Transaction::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM \"player_username_state\" WHERE \"player_username_state\".\"entity_id\" IN ($1) AND \"player_username_state\".\"region\" = $2",
                [1i64.into(), 2i16.into()],
            )]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_module::module_bindings::{
        OffsetCoordinatesSmallMessage, TeleportLocation, TeleportLocationType,
    };
    use sea_orm::{DbBackend, MockDatabase, MockExecResult, Transaction};

    fn player(id: u64) -> PlayerState {
        PlayerState {
            entity_id: id,
            time_played: 10,
            time_signed_in: 10,
            session_start_timestamp: 1,
            sign_in_timestamp: 1,
            signed_in: true,
            traveler_tasks_expiration: 0,
            teleport_location: TeleportLocation {
                location: OffsetCoordinatesSmallMessage {
                    x: 0,
                    z: 0,
                    dimension: 1,
                },
                location_type: TeleportLocationType::BirthLocation,
            },
        }
    }

    fn worker(
        db: sea_orm::DatabaseConnection,
    ) -> (
        PlayerStateWorker,
        tokio::sync::mpsc::Receiver<PlayerStateDbOperation>,
    ) {
        let (outbound, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db, outbound, Default::default());
        let (tx, rx) = crate::websocket::worker_channel::shutdown_channel(&state.shutdown);
        let (db_tx, db_rx) = channel(20);
        (
            PlayerStateWorker {
                rx,
                tx,
                global_app_state: state,
                db_tx,
                batch_size: 1,
                time_limit: Duration::from_millis(10),
                messages: Vec::new(),
                ids: Vec::new(),
                pending_deletes: HashMap::new(),
            },
            db_rx,
        )
    }

    fn empty_db() -> sea_orm::DatabaseConnection {
        MockDatabase::new(DbBackend::Postgres).into_connection()
    }

    fn expire_deletes(worker: &mut PlayerStateWorker) {
        for delete in worker.pending_deletes.values_mut() {
            delete.deadline = Instant::now();
        }
    }

    #[tokio::test]
    async fn live_deletes_survive_batch_flushes_and_only_expire_after_grace() {
        let (mut worker, mut operations) = worker(empty_db());
        let before = Instant::now();
        worker.handle_remove(player(1), 1).await;
        let deadline = worker.pending_deletes[&1].deadline;
        assert!(deadline >= before + PLAYER_DELETE_GRACE);
        worker.handle_remove(player(1), 1).await;
        assert_eq!(worker.pending_deletes[&1].deadline, deadline);
        worker.flush().await;
        worker.reset_batch();
        assert!(operations.try_recv().is_err());
        assert!(!worker.is_idle());
        expire_deletes(&mut worker);
        worker.flush().await;
        assert!(matches!(operations.try_recv().unwrap(),
            PlayerStateDbOperation::DeleteForRegion { ids, region: 1 } if ids == [1]));
        assert!(worker.is_idle());
    }

    #[tokio::test]
    async fn destination_insert_or_update_cancels_source_delete_across_batches() {
        for update in [false, true] {
            let (mut worker, mut operations) = worker(empty_db());
            worker.handle_remove(player(1), 1).await;
            worker.flush().await;
            worker.reset_batch();
            if update {
                worker.handle_update(player(1), 2, player(1)).await;
            } else {
                worker.handle_insert(player(1), 2).await;
            }
            assert!(worker.pending_deletes.is_empty());
            worker.flush().await;
            assert!(matches!(operations.try_recv().unwrap(),
                PlayerStateDbOperation::Upsert(rows) if rows.len() == 1 && *rows[0].region.as_ref() == 2));
            assert!(operations.try_recv().is_err());
        }
    }

    #[tokio::test]
    async fn late_source_remove_preserves_destination_upsert() {
        let (mut worker, mut operations) = worker(empty_db());
        worker.handle_insert(player(1), 2).await;
        worker.handle_remove(player(1), 1).await;
        assert!(worker.pending_deletes.is_empty());
        worker.flush().await;
        assert!(matches!(operations.try_recv().unwrap(),
            PlayerStateDbOperation::Upsert(rows) if rows.len() == 1 && *rows[0].region.as_ref() == 2));
        assert!(operations.try_recv().is_err());
        assert_eq!(
            worker.global_app_state.player_state.get(&1).unwrap().region,
            2
        );
    }

    #[tokio::test]
    async fn same_region_reappearance_also_cancels_deletion() {
        let (mut worker, _) = worker(empty_db());
        worker.handle_remove(player(1), 1).await;
        worker.handle_insert(player(1), 1).await;
        assert!(worker.pending_deletes.is_empty());
    }

    #[tokio::test]
    async fn snapshots_cancel_arrivals_and_delay_missing_rows() {
        let missing = ::entity::player_state::ModelBuilder::new(player(2))
            .with_region(2)
            .build();
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![missing]])
            .into_connection();
        let (mut worker, mut operations) = worker(db);
        worker.handle_remove(player(1), 1).await;
        worker.handle_initial(vec![player(1)], 2).await;
        assert!(!worker.pending_deletes.contains_key(&1));
        assert_eq!(worker.pending_deletes[&2].region, 2);
        worker.flush().await;
        assert!(matches!(
            operations.try_recv().unwrap(),
            PlayerStateDbOperation::Upsert(_)
        ));
        assert!(operations.try_recv().is_err());
        expire_deletes(&mut worker);
        worker.flush_deletes().await;
        assert!(matches!(operations.try_recv().unwrap(),
            PlayerStateDbOperation::DeleteForRegion { ids, region: 2 } if ids == [2]));
    }

    #[tokio::test]
    async fn runner_drains_closed_input_without_shortening_delete_grace() {
        let (worker, mut operations) = worker(empty_db());
        let tx = worker.tx();
        tx.send(SpacetimeUpdateMessages::Remove {
            event: None,
            delete: player(1),
            database_name: 1,
        })
        .unwrap();
        let shutdown = worker.global_app_state.shutdown.clone();
        let before = Instant::now();
        worker.start(&shutdown);
        let task = tokio::spawn(async move {
            shutdown.drain().await;
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(50), operations.recv())
                .await
                .is_err()
        );
        let operation = tokio::time::timeout(
            PLAYER_DELETE_GRACE + Duration::from_secs(2),
            operations.recv(),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(before.elapsed() >= PLAYER_DELETE_GRACE);
        assert!(matches!(operation,
            PlayerStateDbOperation::DeleteForRegion { ids, region: 1 } if ids == [1]));
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn confirmed_deletes_are_scoped_to_the_source_region_in_sql() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_exec_results([MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        let (worker, _) = worker(db);
        delete_multiple_player_state_for_region(&worker.global_app_state, vec![1], 2).await;
        assert_eq!(
            worker.global_app_state.conn.into_transaction_log(),
            [Transaction::from_sql_and_values(
                DbBackend::Postgres,
                "DELETE FROM \"player_state\" WHERE \"player_state\".\"entity_id\" IN ($1) AND \"player_state\".\"region\" = $2",
                [1i64.into(), 2i16.into()],
            )]
        );
    }
}
