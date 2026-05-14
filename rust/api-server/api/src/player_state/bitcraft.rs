use crate::AppState;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::{SpacetimeUpdateMessages, WebSocketMessages};
use game_module::module_bindings::{PlayerState, PlayerUsernameState};
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait, IntoActiveModel, sea_query};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::{
    Receiver, Sender, UnboundedReceiver, UnboundedSender, channel, unbounded_channel,
};

enum PlayerStateDbOperation {
    Upsert(Vec<::entity::player_state::ActiveModel>),
    Delete(Vec<i64>),
    DeleteForRegion {
        ids: Vec<i64>,
        region: entity::shared::Region,
    },
}

enum PlayerUsernameStateDbOperation {
    Upsert(Vec<::entity::player_username_state::ActiveModel>),
    Delete(Vec<i64>),
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
    messages_delete: Vec<i64>,
}

impl PlayerStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_player_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            ids: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
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

    async fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self.db_tx.send(PlayerStateDbOperation::Delete(ids)).await {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerState deletes"
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
                reducer_name,
                ..
            } => {
                self.handle_remove(delete, database_name, reducer_name)
                    .await;
            }
        }
    }

    async fn handle_initial(
        &mut self,
        data: Vec<PlayerState>,
        database_name: entity::shared::Region,
    ) {
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
                local_messages = Vec::with_capacity(self.batch_size + 10);
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

        for chunk_ids in currently_known_player_state
            .into_keys()
            .collect::<Vec<_>>()
            .chunks(1000)
        {
            self.queue_region_deletes(chunk_ids.to_vec(), database_name)
                .await;
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

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", model.signed_in.to_string()),
                ("region", database_name.to_string())
            ]
        )
        .increment(1);

        if self.ids.contains(&model.entity_id) {
            if let Some(index) =
                self.messages
                    .iter()
                    .position(|value: &::entity::player_state::ActiveModel| {
                        value.entity_id.as_ref() == &model.entity_id
                    })
            {
                if model.sign_in_timestamp == 0 {
                    model.sign_in_timestamp =
                        self.messages[index].sign_in_timestamp.clone().unwrap();
                }
                self.messages.remove(index);
            }
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
                .send(WebSocketMessages::TimePlayed {
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
                .send(WebSocketMessages::TimeSignedIn {
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

        if self.ids.contains(&model.entity_id) {
            if let Some(index) = self
                .messages
                .iter()
                .position(|value| value.entity_id.as_ref() == &model.entity_id)
            {
                if model.sign_in_timestamp == 0 {
                    model.sign_in_timestamp =
                        self.messages[index].sign_in_timestamp.clone().unwrap();
                }
                self.messages.remove(index);
            }
        }

        if let Some(index) = self
            .messages_delete
            .iter()
            .position(|value| *value == model.entity_id)
        {
            self.messages_delete.remove(index);
        }

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
            .send(WebSocketMessages::PlayerState(model.clone()));
        self.messages.push(model.into_active_model());
    }

    async fn handle_remove(
        &mut self,
        delete: PlayerState,
        database_name: entity::shared::Region,
        reducer_name: Option<&'static str>,
    ) {
        let model: ::entity::player_state::Model =
            ::entity::player_state::ModelBuilder::new(delete)
                .with_region(database_name)
                .build();
        let id = model.entity_id;

        #[allow(clippy::single_match)]
        match reducer_name {
            Some("transfer_player_delayed") => {
                metrics::gauge!(
                    "players_current_state",
                    &[
                        ("online", model.signed_in.to_string()),
                        ("region", database_name.to_string())
                    ]
                )
                .decrement(1);
                return;
            }
            _ => {}
        }

        if self.ids.contains(&id) {
            if let Some(index) = self
                .messages
                .iter()
                .position(|value| value.entity_id.as_ref() == &model.entity_id)
            {
                self.messages.remove(index);
            }
        }

        metrics::gauge!(
            "players_current_state",
            &[
                ("online", model.signed_in.to_string()),
                ("region", database_name.to_string())
            ]
        )
        .decrement(1);

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

        tracing::debug!("PlayerState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for PlayerStateWorker {
    type Entity = PlayerState;

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
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
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

async fn delete_multiple_player_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::player_state::Entity::delete_many()
            .filter(::entity::player_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                PlayerState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete PlayerState"
            );
        }
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
            .filter(::entity::player_state::Column::Region.eq(region.clone()))
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

    tokio::spawn(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                PlayerStateDbOperation::Upsert(messages) => {
                    insert_multiple_player_state(&global_app_state, &on_conflict, messages).await;
                }
                PlayerStateDbOperation::Delete(ids) => {
                    delete_multiple_player_state(&global_app_state, ids).await;
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
    ids: Vec<i64>,
    messages_delete: Vec<i64>,
}

impl PlayerUsernameStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_player_username_state_db_worker(global_app_state.clone());
        let (tx, rx) = unbounded_channel();

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            messages: Vec::with_capacity(batch_size + 10),
            ids: Vec::with_capacity(batch_size + 10),
            messages_delete: Vec::with_capacity(batch_size + 10),
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

    async fn queue_deletes(&self, ids: Vec<i64>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(PlayerUsernameStateDbOperation::Delete(ids))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue PlayerUsernameState deletes"
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
                    use std::collections::hash_map::Entry;
                    match currently_known_player_username_state.entry(model.entity_id) {
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
                        self.queue_upserts(std::mem::take(&mut local_messages))
                            .await;
                        local_messages = Vec::with_capacity(self.batch_size + 10);
                    }
                }
                if !local_messages.is_empty() {
                    self.queue_upserts(local_messages).await;
                }

                for chunk_ids in currently_known_player_username_state
                    .into_keys()
                    .collect::<Vec<_>>()
                    .chunks(1000)
                {
                    self.queue_region_deletes(chunk_ids.to_vec(), database_name)
                        .await;
                }
            }
            SpacetimeUpdateMessages::Insert {
                new, database_name, ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(new)
                        .with_region(database_name)
                        .build();

                if let Some(index) = self
                    .messages_delete
                    .iter()
                    .position(|value| *value == model.entity_id)
                {
                    self.messages_delete.remove(index);
                }
                self.ids.push(model.entity_id);
                self.messages.push(model.into_active_model());
            }
            SpacetimeUpdateMessages::Update {
                new, database_name, ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(new)
                        .with_region(database_name)
                        .build();
                if let Some(index) = self
                    .messages_delete
                    .iter()
                    .position(|value| *value == model.entity_id)
                {
                    self.messages_delete.remove(index);
                }
                self.ids.push(model.entity_id);
                self.messages.push(model.into_active_model());
            }
            SpacetimeUpdateMessages::Remove {
                delete,
                database_name,
                reducer_name,
                ..
            } => {
                let model: ::entity::player_username_state::Model =
                    ::entity::player_username_state::ModelBuilder::new(delete)
                        .with_region(database_name)
                        .build();
                let id = model.entity_id;

                #[allow(clippy::single_match)]
                match reducer_name {
                    Some("transfer_player_delayed") => {
                        return;
                    }
                    _ => {}
                }

                if self.ids.contains(&id) {
                    if let Some(index) = self
                        .messages
                        .iter()
                        .position(|value| value.entity_id.as_ref() == &model.entity_id)
                    {
                        self.messages.remove(index);
                    }
                }

                self.messages_delete.push(id);
            }
        }
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

        tracing::debug!("PlayerUsernameState::Remove");
        let messages_delete = std::mem::replace(
            &mut self.messages_delete,
            Vec::with_capacity(self.batch_size + 10),
        );
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for PlayerUsernameStateWorker {
    type Entity = PlayerUsernameState;
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
        self.messages.len() >= self.batch_size || self.messages_delete.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.messages.is_empty() && self.messages_delete.is_empty()
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

async fn delete_multiple_player_username_state(global_app_state: &AppState, ids: Vec<i64>) {
    for chunk_ids in ids.chunks(1000) {
        let chunk_ids = chunk_ids.to_vec();
        if let Err(error) = ::entity::player_username_state::Entity::delete_many()
            .filter(::entity::player_username_state::Column::EntityId.is_in(chunk_ids.clone()))
            .exec(&global_app_state.conn)
            .await
        {
            let chunk_ids_str: Vec<String> = chunk_ids.iter().map(|id| id.to_string()).collect();
            tracing::error!(
                PlayerUsernameState = chunk_ids_str.join(","),
                error = error.to_string(),
                "Could not delete PlayerUsernameState"
            );
        }
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
            .filter(::entity::player_username_state::Column::Region.eq(region.clone()))
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

    tokio::spawn(async move {
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
                PlayerUsernameStateDbOperation::Delete(ids) => {
                    delete_multiple_player_username_state(&global_app_state, ids).await;
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
