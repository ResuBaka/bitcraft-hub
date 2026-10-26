use crate::AppState;
use crate::leaderboard::experience_to_level;
use crate::websocket::batched_worker::BatchedWorker;
use crate::websocket::worker_channel::{UnboundedReceiver, UnboundedSender};
use crate::websocket::{OutboundWebSocketMessages, SpacetimeUpdateMessages};
use entity::shared::Region;
use game_module::module_bindings::{ExperienceStack, ExperienceState};
use migration::{OnConflict, sea_query};
use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, IntoActiveModel, QueryFilter};
use std::collections::{HashMap, HashSet};
use std::ops::AddAssign;
use std::time::Duration;
use tokio::sync::mpsc::{Sender, channel};

enum ExperienceStateDbOperation {
    Upsert(Vec<::entity::experience_state::ActiveModel>),
    Delete(Vec<(i64, i32)>),
    Barrier(tokio::sync::oneshot::Sender<()>),
}

/// The database key deliberately excludes region, just like the table's primary
/// key. Keep only the last pending operation for each key, across all regions.
#[derive(Default)]
struct PendingExperienceWrites {
    upserts: HashMap<(i64, i32), ::entity::experience_state::Model>,
    deletes: HashSet<(i64, i32)>,
}

impl PendingExperienceWrites {
    fn upsert(&mut self, model: ::entity::experience_state::Model) {
        let key = (model.entity_id, model.skill_id);
        self.deletes.remove(&key);
        self.upserts.insert(key, model);
    }

    fn delete(&mut self, key: (i64, i32)) {
        self.upserts.remove(&key);
        self.deletes.insert(key);
    }
}

#[derive(Clone, Copy)]
struct SkillCalculation {
    level: i32,
    category: Option<i32>,
}

pub(crate) struct ExperienceStateWorker {
    rx: UnboundedReceiver<SpacetimeUpdateMessages<ExperienceState>>,
    tx: UnboundedSender<SpacetimeUpdateMessages<ExperienceState>>,
    global_app_state: AppState,
    batch_size: usize,
    time_limit: Duration,
    db_tx: Sender<ExperienceStateDbOperation>,
    pending: PendingExperienceWrites,
}

/// Flatten lazily: do not allocate a second region-wide experience row vector.
fn initial_experience_rows(
    state: &AppState,
    data: Vec<ExperienceState>,
    region: Region,
) -> impl Iterator<Item = ::entity::experience_state::Model> + Send + '_ {
    data.into_iter().flat_map(move |value| {
        let id = value.entity_id as i64;
        let mut total_exp = 0;
        let mut total_level = 0;
        let mut profession_exp = 0;
        let mut adventure_exp = 0;
        for stack in &value.experience_stacks {
            total_exp += i64::from(stack.quantity);
            total_level += i64::from(experience_to_level(i64::from(stack.quantity)));
            if let Some(skill) = state.skill_desc.get(&i64::from(stack.skill_id)) {
                match skill.skill_category {
                    1 => profession_exp += i64::from(stack.quantity),
                    2 => adventure_exp += i64::from(stack.quantity),
                    _ => {}
                }
            }
        }
        // A snapshot must also initialize players absent from the startup DB
        // load. Board updates already skip scores that have not changed.
        if state.ranking_system.global_leaderboard.get_value(&id) != Some(total_exp) {
            state
                .ranking_system
                .global_leaderboard
                .update(id, total_exp);
            let xp_per_hour = state
                .player_state
                .get(&id)
                .filter(|player| player.time_signed_in >= 3_600)
                .map_or(0, |player| {
                    total_exp / (i64::from(player.time_signed_in) / 3_600)
                });
            state.ranking_system.xp_per_hour.update(id, xp_per_hour);
        }
        state
            .ranking_system
            .level_leaderboard
            .update(id, total_level);
        state
            .ranking_system
            .adventure_leaderboard
            .update(id, adventure_exp);
        state
            .ranking_system
            .profession_leaderboard
            .update(id, profession_exp);
        value
            .experience_stacks
            .into_iter()
            .map(move |stack| ::entity::experience_state::Model {
                entity_id: id,
                skill_id: stack.skill_id,
                experience: stack.quantity,
                region,
            })
    })
}

#[cfg(test)]
mod initial_tests {
    use super::*;
    use game_module::module_bindings::ExperienceStack;

    fn experience(player: u64, skills: &[(i32, i32)]) -> ExperienceState {
        ExperienceState {
            entity_id: player,
            experience_stacks: skills
                .iter()
                .map(|&(skill_id, quantity)| ExperienceStack { skill_id, quantity })
                .collect(),
        }
    }

    #[test]
    fn level_conversion_matches_legacy_at_every_threshold_and_extreme() {
        use crate::leaderboard::EXPERIENCE_PER_LEVEL;
        assert!(
            EXPERIENCE_PER_LEVEL
                .windows(2)
                .all(|pair| pair[0].1 < pair[1].1)
        );
        let values = [i64::MIN, -1, 0, 1, i64::MAX].into_iter().chain(
            EXPERIENCE_PER_LEVEL
                .iter()
                .flat_map(|(_, xp)| [xp - 1, *xp, xp + 1]),
        );
        for xp in values {
            let legacy = EXPERIENCE_PER_LEVEL
                .iter()
                .rev()
                .find(|(_, threshold)| xp >= *threshold)
                .map_or(100, |(level, _)| *level);
            assert_eq!(experience_to_level(xp), legacy, "XP={xp}");
        }
    }

    #[test]
    fn pending_writes_keep_only_the_latest_operation_for_each_database_key() {
        let mut pending = PendingExperienceWrites::default();
        let row = |player, skill, xp, region| ::entity::experience_state::Model {
            entity_id: player,
            skill_id: skill,
            experience: xp,
            region,
        };
        pending.upsert(row(1, 10, 100, 7));
        pending.upsert(row(1, 11, 200, 7));
        pending.upsert(row(2, 10, 300, 7));
        for xp in 101..=1000 {
            pending.upsert(row(1, 10, xp, 8));
        }
        assert_eq!(pending.upserts.len(), 3);
        assert_eq!(pending.upserts[&(1, 10)], row(1, 10, 1000, 8));
        pending.delete((1, 10));
        pending.delete((1, 10));
        assert_eq!(pending.upserts.len(), 2);
        assert_eq!(pending.deletes.len(), 1);
        pending.upsert(row(1, 10, 1200, 9));
        assert!(pending.deletes.is_empty());
        assert_eq!(pending.upserts[&(1, 10)], row(1, 10, 1200, 9));
        assert_eq!(pending.upserts[&(1, 11)], row(1, 11, 200, 7));
        assert_eq!(pending.upserts[&(2, 10)], row(2, 10, 300, 7));
    }

    #[tokio::test]
    async fn flushes_preserve_operation_order_latest_xp_and_reusable_indexes() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let mut worker = ExperienceStateWorker::new(state, 2, Duration::from_secs(10));
        let (db_tx, mut operations) = channel(5);
        worker.db_tx = db_tx;
        worker
            .handle_insert(experience(1, &[(10, 100), (11, 100)]), 7)
            .await;
        assert!(worker.should_flush());
        worker
            .handle_update(
                experience(1, &[(10, 200), (11, 100)]),
                8,
                experience(1, &[(10, 100), (11, 100)]),
            )
            .await;
        worker
            .handle_update(
                experience(1, &[(10, 300), (11, 100)]),
                9,
                experience(1, &[(10, 200), (11, 100)]),
            )
            .await;
        worker
            .handle_remove(experience(1, &[(11, 100), (11, 100)]), 7)
            .await;
        let capacity = worker.pending.upserts.capacity();
        worker.flush().await;
        assert!(worker.is_idle());
        assert_eq!(worker.pending.upserts.capacity(), capacity);
        match operations.try_recv().unwrap() {
            ExperienceStateDbOperation::Upsert(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    rows[0],
                    ::entity::experience_state::Model {
                        entity_id: 1,
                        skill_id: 10,
                        experience: 300,
                        region: 9
                    }
                    .into_active_model()
                );
            }
            _ => panic!("upserts precede deletes"),
        }
        match operations.try_recv().unwrap() {
            ExperienceStateDbOperation::Delete(keys) => assert_eq!(keys, [(1, 11)]),
            _ => panic!("expected delete"),
        }
        worker.handle_insert(experience(1, &[(11, 400)]), 8).await;
        worker.flush().await;
        assert!(matches!(
            operations.try_recv().unwrap(),
            ExperienceStateDbOperation::Upsert(_)
        ));
        worker.flush().await;
        assert!(
            operations.try_recv().is_err(),
            "empty flushes do not queue operations"
        );
    }

    #[tokio::test]
    async fn snapshot_drains_indexed_pending_writes_before_reading_keys() {
        let model = ::entity::experience_state::Model {
            entity_id: 1,
            skill_id: 10,
            experience: 200,
            region: 7,
        };
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([vec![model], vec![]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let mut worker = ExperienceStateWorker::new(state, 100, Duration::from_secs(10));
        worker
            .handle_update(experience(1, &[(10, 200)]), 7, experience(1, &[(10, 100)]))
            .await;
        worker.handle_initial(vec![], 7).await;
        assert!(worker.is_idle());
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 2);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
        assert!(format!("{:?}", log[1]).contains("SELECT"));
    }

    #[tokio::test]
    async fn initial_sync_populates_missing_boards_even_when_database_rows_are_unchanged() {
        use axum::extract::{Path, State};
        const ID: i64 = 1297036692699947659;
        let rows = [(10, 1200), (11, 700)].map(|(skill_id, experience)| {
            ::entity::experience_state::Model {
                entity_id: ID,
                skill_id,
                experience,
                region: 7,
            }
        });
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([rows
                .iter()
                .map(|row| {
                    std::collections::BTreeMap::<String, sea_orm::Value>::from([
                        ("0".into(), ID.into()),
                        ("1".into(), row.skill_id.into()),
                    ])
                })
                .collect::<Vec<_>>()])
            .append_query_results([rows.to_vec()])
            .append_query_results([vec![::entity::player_username_state::Model {
                entity_id: ID,
                username: "Bris".into(),
                region: 7,
            }]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        for (id, category) in [(10, 1), (11, 2)] {
            state.skill_desc.insert(
                id,
                ::entity::skill_desc::Model {
                    id,
                    skill: id as i32,
                    name: format!("skill-{id}"),
                    skill_category: category,
                    description: String::new(),
                    icon_asset_name: String::new(),
                    title: String::new(),
                },
            );
            state.ranking_system.ensure_skill_leaderboard(id);
        }
        state.player_state.insert(
            ID,
            ::entity::player_state::Model {
                entity_id: ID,
                region: 7,
                time_played: 7200,
                time_signed_in: 7200,
                session_start_timestamp: 0,
                sign_in_timestamp: 0,
                signed_in: false,
                traveler_tasks_expiration: 0,
                teleport_location: ::entity::player_state::TeleportLocation {
                    location: ::entity::player_state::OffsetCoordinatesSmallMessage {
                        x: 0,
                        z: 0,
                        dimension: 1,
                    },
                    location_type: ::entity::player_state::TeleportLocationType::BirthLocation,
                },
            },
        );
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        worker
            .handle_initial(vec![experience(ID as u64, &[(10, 1200), (11, 700)])], 7)
            .await;
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&ID),
            Some(1900)
        );
        assert_eq!(
            state.ranking_system.level_leaderboard.get_value(&ID),
            Some(5)
        );
        assert_eq!(
            state.ranking_system.profession_leaderboard.get_value(&ID),
            Some(1200)
        );
        assert_eq!(
            state.ranking_system.adventure_leaderboard.get_value(&ID),
            Some(700)
        );
        assert_eq!(state.ranking_system.xp_per_hour.get_value(&ID), Some(950));
        for row in rows {
            assert_eq!(
                state
                    .ranking_system
                    .skill_leaderboard(i64::from(row.skill_id))
                    .unwrap()
                    .get_value(&ID),
                Some(i64::from(row.experience))
            );
        }
        state.ranking_system.shutdown().await;
        let response = super::super::player_leaderboard(State(state), Path(ID))
            .await
            .unwrap()
            .0
            .0;
        assert!(
            matches!(&response["Experience"], super::super::RankType::Experience(value) if value.experience == 1900 && value.rank == 1)
        );
        assert!(
            matches!(&response["Level"], super::super::RankType::Level(value) if value.level == 5 && value.rank == 1)
        );
        let queries = db.into_transaction_log();
        assert_eq!(queries.len(), 3);
        assert!(
            queries
                .iter()
                .all(|query| !format!("{query:?}").contains("INSERT INTO"))
        );
    }

    #[tokio::test]
    async fn inserted_players_get_skill_and_total_level_boards_without_an_xp_update() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(Default::default(), tx, Default::default());
        let board = state.ranking_system.ensure_skill_leaderboard(10);
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        worker.handle_insert(experience(1, &[(10, 1200)]), 7).await;
        assert_eq!(board.get_value(&1), Some(1200));
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&1),
            Some(1200)
        );
        assert_eq!(
            state.ranking_system.level_leaderboard.get_value(&1),
            Some(3)
        );
    }

    #[tokio::test]
    async fn reordered_skills_keep_notification_order_and_fresh_description_data() {
        let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        for (id, category) in [(10, 1), (11, 2), (12, 0)] {
            state.skill_desc.insert(
                id,
                ::entity::skill_desc::Model {
                    id,
                    skill: id as i32,
                    name: format!("skill-{id}"),
                    skill_category: category,
                    description: String::new(),
                    icon_asset_name: String::new(),
                    title: String::new(),
                },
            );
        }
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        let old = experience(1, &[(10, 500), (11, 600), (12, 20)]);
        let new = experience(1, &[(11, 700), (10, 600), (12, 20), (13, 25)]);
        worker.handle_update(new.clone(), 7, old).await;
        let mut events = Vec::new();
        while let Ok(event) = notifications.try_recv() {
            events.push(match event {
                OutboundWebSocketMessages::Level {
                    level, skill_name, ..
                } => {
                    assert_eq!(level, 2);
                    format!("level:{skill_name}")
                }
                OutboundWebSocketMessages::Experience {
                    experience,
                    skill_name,
                    ..
                } => format!("xp:{skill_name}:{experience}"),
                OutboundWebSocketMessages::TotalExperience { experience, .. } => {
                    format!("total:{experience}")
                }
                OutboundWebSocketMessages::TotalAdventureExperience { experience, .. } => {
                    format!("adventure:{experience}")
                }
                OutboundWebSocketMessages::TotalProfessionExperience { experience, .. } => {
                    format!("profession:{experience}")
                }
                _ => panic!("unexpected notification"),
            });
        }
        assert_eq!(
            events,
            [
                "level:skill-10",
                "xp:skill-10:600",
                "xp:skill-11:700",
                "total:1345",
                "adventure:700",
                "profession:600"
            ]
        );
        assert_eq!(
            state.ranking_system.level_leaderboard.get_value(&1),
            Some(6)
        );
        assert_eq!(worker.pending.upserts.len(), 2);
        assert!(
            !worker.pending.upserts.contains_key(&(1, 13)),
            "new/removed-skill persistence policy is not changed by this optimization"
        );

        {
            let mut skill = state.skill_desc.get_mut(&10).unwrap();
            skill.name = "renamed".into();
            skill.skill_category = 2;
        }
        worker
            .handle_update(
                experience(1, &[(11, 700), (10, 1200), (12, 20), (13, 25)]),
                7,
                new,
            )
            .await;
        assert!(
            matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::Level { level: 3, skill_name, .. } if skill_name == "renamed")
        );
        assert!(
            matches!(notifications.try_recv().unwrap(), OutboundWebSocketMessages::Experience { experience: 1200, skill_name, .. } if skill_name == "renamed")
        );
        assert_eq!(
            state.ranking_system.adventure_leaderboard.get_value(&1),
            Some(1900)
        );
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::TotalExperience {
                experience: 1945,
                ..
            }
        ));
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::TotalAdventureExperience {
                experience: 1900,
                ..
            }
        ));
        assert!(notifications.try_recv().is_err());

        worker
            .handle_update(experience(1, &[(10, 100)]), 7, experience(1, &[(10, 1200)]))
            .await;
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::Level { level: 1, .. }
        ));
        while let Ok(event) = notifications.try_recv() {
            assert!(
                !matches!(event, OutboundWebSocketMessages::Experience { .. }),
                "XP decreases do not add skill-XP notifications"
            );
        }

        // A removed skill has no new-side calculation. Its old XP/level must
        // still contribute to the aggregate comparison, without a new policy
        // for deleting skill rows as part of an Update event.
        worker
            .handle_update(
                experience(1, &[(11, 700)]),
                7,
                experience(1, &[(10, 100), (11, 700)]),
            )
            .await;
        assert_eq!(
            state.ranking_system.level_leaderboard.get_value(&1),
            Some(2)
        );
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::TotalExperience {
                experience: 700,
                ..
            }
        ));
        assert!(matches!(
            notifications.try_recv().unwrap(),
            OutboundWebSocketMessages::TotalAdventureExperience {
                experience: 700,
                ..
            }
        ));
        assert!(notifications.try_recv().is_err());
        assert!(worker.pending.deletes.is_empty());
    }

    #[tokio::test]
    async fn combined_update_pass_handles_aligned_stacks_tails_and_equal_length_replacements() {
        let cases: &[(&[(i32, i32)], &[(i32, i32)])] = &[
            (&[(10, 500), (11, 600)], &[(10, 600), (11, 700)]),
            (&[(10, 500)], &[(10, 600), (11, 700)]),
            (&[(10, 500), (11, 600)], &[(10, 600)]),
            (&[(10, 500), (11, 600)], &[(10, 600), (12, 700)]),
            (&[(10, 500), (11, 600)], &[(11, 700), (10, 600)]),
            (&[], &[(10, 600), (11, 700)]),
            (&[(10, 500), (11, 600)], &[]),
            (&[], &[]),
            (&[(10, 500), (11, 600)], &[(10, 500), (11, 600)]),
            (&[(10, 1200), (11, 600)], &[(10, 100), (11, 600)]),
        ];
        for &(old, new) in cases {
            let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
            let state = AppState::new(Default::default(), tx, Default::default());
            for (id, category) in [(10, 1), (11, 2), (12, 0)] {
                state.skill_desc.insert(
                    id,
                    ::entity::skill_desc::Model {
                        id,
                        skill: id as i32,
                        name: format!("skill-{id}"),
                        skill_category: category,
                        description: String::new(),
                        icon_asset_name: String::new(),
                        title: String::new(),
                    },
                );
            }
            let totals = |skills: &[(i32, i32)]| {
                [
                    skills.iter().map(|(_, xp)| i64::from(*xp)).sum(),
                    skills
                        .iter()
                        .map(|(_, xp)| i64::from(experience_to_level(i64::from(*xp))))
                        .sum(),
                    skills
                        .iter()
                        .filter(|(skill, _)| *skill == 10)
                        .map(|(_, xp)| i64::from(*xp))
                        .sum(),
                    skills
                        .iter()
                        .filter(|(skill, _)| *skill == 11)
                        .map(|(_, xp)| i64::from(*xp))
                        .sum(),
                ]
            };
            let boards = [
                &state.ranking_system.global_leaderboard,
                &state.ranking_system.level_leaderboard,
                &state.ranking_system.profession_leaderboard,
                &state.ranking_system.adventure_leaderboard,
            ];
            for (board, total) in boards.iter().zip(totals(old)) {
                board.update(1, total);
            }
            let mut worker =
                ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
            worker
                .handle_update(experience(1, new), 7, experience(1, old))
                .await;
            for (board, total) in boards.iter().zip(totals(new)) {
                assert_eq!(board.get_value(&1), Some(total), "old={old:?}, new={new:?}");
            }
            let mut expected = Vec::new();
            let mut expected_writes = HashMap::new();
            for &(skill, old_xp) in old {
                if let Some(&(_, xp)) = new.iter().find(|(id, _)| *id == skill) {
                    if experience_to_level(i64::from(xp)) != experience_to_level(i64::from(old_xp))
                    {
                        expected.push(format!("level:skill-{skill}"));
                    }
                    if xp > old_xp {
                        expected_writes.insert((1, skill), xp);
                        if skill != 12 {
                            expected.push(format!("xp:skill-{skill}"));
                        }
                    }
                }
            }
            let mut actual = Vec::new();
            while let Ok(event) = notifications.try_recv() {
                match event {
                    OutboundWebSocketMessages::Level { skill_name, .. } => {
                        actual.push(format!("level:{skill_name}"))
                    }
                    OutboundWebSocketMessages::Experience { skill_name, .. } => {
                        actual.push(format!("xp:{skill_name}"))
                    }
                    _ => {}
                }
            }
            assert_eq!(actual, expected, "old={old:?}, new={new:?}");
            assert_eq!(
                worker
                    .pending
                    .upserts
                    .iter()
                    .map(|(&key, row)| (key, row.experience))
                    .collect::<HashMap<_, _>>(),
                expected_writes
            );
            assert!(worker.pending.deletes.is_empty());
        }
    }

    #[test]
    fn experience_rows_are_flattened_lazily_without_losing_totals() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        state.ranking_system.global_leaderboard.update(1, 1);
        state.ranking_system.global_leaderboard.update(2, 1);
        let data = vec![
            ExperienceState {
                entity_id: 1,
                experience_stacks: vec![
                    ExperienceStack {
                        skill_id: 10,
                        quantity: 100,
                    },
                    ExperienceStack {
                        skill_id: 11,
                        quantity: 200,
                    },
                ],
            },
            ExperienceState {
                entity_id: 2,
                experience_stacks: vec![ExperienceStack {
                    skill_id: 10,
                    quantity: 400,
                }],
            },
        ];
        let mut rows = initial_experience_rows(&state, data, 7);
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&1),
            Some(1)
        );
        let first = rows.next().unwrap();
        assert_eq!(
            (
                first.entity_id,
                first.skill_id,
                first.experience,
                first.region
            ),
            (1, 10, 100, 7)
        );
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&1),
            Some(300)
        );
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&2),
            Some(1)
        );
        assert_eq!(rows.next().unwrap().skill_id, 11);
        assert_eq!(rows.next().unwrap().experience, 400);
        assert_eq!(
            state.ranking_system.global_leaderboard.get_value(&2),
            Some(400)
        );
        assert!(rows.next().is_none());
    }

    #[tokio::test]
    async fn unchanged_event_does_not_register_an_unused_player_handle() {
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        assert_eq!(state.ranking_system.players.len(), 0);
        worker
            .handle_update(experience(1, &[(10, 100)]), 7, experience(1, &[(10, 100)]))
            .await;
        assert_eq!(state.ranking_system.players.len(), 0);
    }

    #[tokio::test]
    async fn debug_rank_logging_does_not_change_notification_payloads_or_order() {
        let mut runs = Vec::new();
        for level in [tracing::Level::INFO, tracing::Level::DEBUG] {
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(level)
                .with_writer(std::io::sink)
                .finish();
            let _guard = tracing::subscriber::set_default(subscriber);
            let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
            let state = AppState::new(
                sea_orm::DatabaseConnection::default(),
                tx,
                Default::default(),
            );
            state.skill_desc.insert(
                10,
                ::entity::skill_desc::Model {
                    id: 10,
                    skill: 10,
                    name: "Mining".into(),
                    skill_category: 1,
                    description: String::new(),
                    icon_asset_name: String::new(),
                    title: String::new(),
                },
            );
            let mut worker = ExperienceStateWorker::new(state, 100, Duration::from_secs(10));
            worker
                .handle_update(experience(1, &[(10, 600)]), 7, experience(1, &[(10, 500)]))
                .await;
            let mut events = Vec::new();
            while let Ok(event) = notifications.try_recv() {
                let mut event = serde_json::to_value(event).unwrap();
                // The asynchronous publisher may run at different instants in
                // the two runs; compare notification data independently of rank.
                if let Some(content) = event
                    .get_mut("c")
                    .and_then(serde_json::Value::as_object_mut)
                {
                    content.remove("rank");
                }
                events.push(event);
            }
            assert_eq!(events.len(), 4);
            runs.push(events);
        }
        assert_eq!(runs[0], runs[1]);
    }

    #[tokio::test]
    async fn live_updates_keep_each_notification_and_immediate_scores() {
        let (tx, mut notifications) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        let row = |quantity| ExperienceState {
            entity_id: 1,
            experience_stacks: vec![ExperienceStack {
                skill_id: 10,
                quantity,
            }],
        };
        for (old, new) in [(100, 200), (200, 300)] {
            worker.handle_update(row(new), 7, row(old)).await;
            assert_eq!(
                state.ranking_system.global_leaderboard.get_value(&1),
                Some(i64::from(new))
            );
            match notifications
                .try_recv()
                .expect("notify without waiting for a batch or ranking publication")
            {
                OutboundWebSocketMessages::TotalExperience {
                    experience,
                    user_id,
                    ..
                } => {
                    assert_eq!(user_id, 1);
                    assert_eq!(experience, new as u64);
                }
                _ => panic!("expected total experience notification"),
            }
        }
        assert!(notifications.try_recv().is_err());
    }

    #[tokio::test]
    async fn insert_keeps_xp_per_hour_for_online_and_offline_players() {
        use entity::player_state::{
            Model, OffsetCoordinatesSmallMessage, TeleportLocation, TeleportLocationType,
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(
            sea_orm::DatabaseConnection::default(),
            tx,
            Default::default(),
        );
        let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
        for (id, signed_in, previous) in
            [(1, true, None), (2, false, Some(50)), (3, true, Some(200))]
        {
            state.player_state.insert(
                id,
                Model {
                    teleport_location: TeleportLocation {
                        location: OffsetCoordinatesSmallMessage {
                            x: 0,
                            z: 0,
                            dimension: 0,
                        },
                        location_type: TeleportLocationType::BirthLocation,
                    },
                    entity_id: id,
                    time_played: 0,
                    session_start_timestamp: 0,
                    time_signed_in: 7200,
                    sign_in_timestamp: 0,
                    signed_in,
                    traveler_tasks_expiration: 0,
                    region: 7,
                },
            );
            if let Some(previous) = previous {
                state.ranking_system.global_leaderboard.update(id, previous);
            }
            worker
                .handle_insert(
                    ExperienceState {
                        entity_id: id as u64,
                        experience_stacks: vec![ExperienceStack {
                            skill_id: 10,
                            quantity: 100,
                        }],
                    },
                    7,
                )
                .await;
            assert_eq!(state.ranking_system.xp_per_hour.get_value(&id), Some(50));
            assert_eq!(
                state.ranking_system.global_leaderboard.get_value(&id),
                Some(std::cmp::max(previous.unwrap_or(0), 100))
            );
        }
    }

    #[tokio::test]
    async fn initial_sync_barrier_waits_for_preceding_live_writes() {
        let model = ::entity::experience_state::Model {
            entity_id: 1,
            skill_id: 10,
            experience: 100,
            region: 7,
        };
        let db = sea_orm::MockDatabase::new(sea_orm::DbBackend::Postgres)
            .append_query_results([vec![model.clone()]])
            .into_connection();
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let state = AppState::new(db.clone(), tx, Default::default());
        let worker = start_experience_state_db_worker(state);
        worker
            .send(ExperienceStateDbOperation::Upsert(vec![
                model.into_active_model(),
            ]))
            .await
            .unwrap();
        let (done, drained) = tokio::sync::oneshot::channel();
        worker
            .send(ExperienceStateDbOperation::Barrier(done))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(1), drained)
            .await
            .unwrap()
            .unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(format!("{:?}", log[0]).contains("INSERT INTO"));
    }
}

fn start_experience_state_db_worker(
    global_app_state: AppState,
) -> Sender<ExperienceStateDbOperation> {
    let (tx, mut rx) = channel(5);
    let on_conflict = sea_query::OnConflict::columns([
        ::entity::experience_state::Column::EntityId,
        ::entity::experience_state::Column::SkillId,
    ])
    .update_columns([
        ::entity::experience_state::Column::Experience,
        ::entity::experience_state::Column::Region,
    ])
    .to_owned();

    global_app_state.shutdown.clone().spawn_worker(async move {
        while let Some(operation) = rx.recv().await {
            match operation {
                ExperienceStateDbOperation::Upsert(messages) => {
                    insert_multiple_experience_state(&global_app_state, &on_conflict, messages)
                        .await;
                }
                ExperienceStateDbOperation::Delete(ids) => {
                    tracing::debug!("ExperienceState::Remove");
                    for chunk_ids in ids.chunks(100) {
                        let mut query = sea_query::Condition::any();
                        for (entity_id, skill_id) in chunk_ids {
                            query = query.add(
                                ::entity::experience_state::Column::EntityId
                                    .eq(*entity_id)
                                    .and(::entity::experience_state::Column::SkillId.eq(*skill_id)),
                            );
                        }

                        let chunk_ids = chunk_ids.to_vec();
                        if let Err(error) = ::entity::experience_state::Entity::delete_many()
                            .filter(query)
                            .exec(&global_app_state.conn)
                            .await
                        {
                            let chunk_ids_str: Vec<String> = chunk_ids
                                .iter()
                                .map(|(entity_id, skill_id)| format!("{}:{}", entity_id, skill_id))
                                .collect();
                            tracing::error!(
                                ExperienceState = chunk_ids_str.join(","),
                                error = error.to_string(),
                                "Could not delete ExperienceState"
                            );
                        }
                    }
                }
                ExperienceStateDbOperation::Barrier(done) => {
                    let _ = done.send(());
                }
            }
        }
    });

    tx
}

impl ExperienceStateWorker {
    pub(crate) fn new(global_app_state: AppState, batch_size: usize, time_limit: Duration) -> Self {
        let db_tx = start_experience_state_db_worker(global_app_state.clone());
        let (tx, rx) =
            crate::websocket::worker_channel::shutdown_channel(&global_app_state.shutdown);

        Self {
            rx,
            tx,
            global_app_state,
            batch_size,
            time_limit,
            db_tx,
            pending: PendingExperienceWrites::default(),
        }
    }

    async fn queue_upserts(&self, messages: Vec<::entity::experience_state::ActiveModel>) {
        if messages.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::Upsert(messages))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState upserts"
            );
        }
    }

    async fn queue_deletes(&self, ids: Vec<(i64, i32)>) {
        if ids.is_empty() {
            return;
        }

        if let Err(error) = self
            .db_tx
            .send(ExperienceStateDbOperation::Delete(ids))
            .await
        {
            tracing::error!(
                error = error.to_string(),
                "Could not queue ExperienceState deletes"
            );
        }
    }

    async fn process_message(&mut self, msg: SpacetimeUpdateMessages<ExperienceState>) {
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
        data: Vec<ExperienceState>,
        database_name: entity::shared::Region,
    ) {
        tracing::debug!("Processed Initial ExperienceState {}", data.len());
        // Drain previously queued live writes before comparing a newer snapshot.
        self.flush_messages().await;
        self.flush_deletes().await;
        let (done, drained) = tokio::sync::oneshot::channel();
        if self
            .db_tx
            .send(ExperienceStateDbOperation::Barrier(done))
            .await
            .is_err()
            || drained.await.is_err()
        {
            tracing::error!(
                region = database_name,
                "Experience persistence worker stopped before initial sync"
            );
            return;
        }
        let on_conflict = sea_query::OnConflict::columns([
            ::entity::experience_state::Column::EntityId,
            ::entity::experience_state::Column::SkillId,
        ])
        .update_columns([
            ::entity::experience_state::Column::Experience,
            ::entity::experience_state::Column::Region,
        ])
        .to_owned();
        let state = &self.global_app_state;
        if let Err(error) = crate::websocket::initial_sync::reconcile::<
            ::entity::experience_state::Entity,
            _,
            _,
            _,
            _,
            _,
        >(
            &state.conn,
            ::entity::experience_state::Column::Region,
            database_name,
            &on_conflict,
            self.batch_size,
            initial_experience_rows(state, data, database_name),
            |model| (model.entity_id, model.skill_id),
            |keys| {
                crate::websocket::initial_sync::composite_filter(
                    ::entity::experience_state::Column::EntityId,
                    ::entity::experience_state::Column::SkillId,
                    keys,
                )
            },
            |model, _changed| {
                // Database equality does not imply that the in-memory board
                // contains this player (for example after a cold start).
                if let Some(board) = state
                    .ranking_system
                    .skill_leaderboard(model.skill_id as i64)
                {
                    board.update(model.entity_id, model.experience as i64);
                }
            },
        )
        .await
        {
            tracing::error!(%error, region = database_name, "Could not reconcile initial experience");
        }
        tracing::debug!(
            "Processed Initial ExperienceState {}",
            database_name.to_string()
        );
    }

    async fn handle_insert(&mut self, new: ExperienceState, database_name: entity::shared::Region) {
        let id = new.entity_id as i64;
        let mut total_exp = 0;
        let mut total_level = 0;
        let mut profession_exp = 0;
        let mut adventure_exp = 0;
        new.experience_stacks.iter().for_each(|es| {
            total_exp.add_assign(es.quantity as i64);
            total_level += i64::from(experience_to_level(i64::from(es.quantity)));
            if let Some(skill_desc) = self.global_app_state.skill_desc.get(&(es.skill_id as i64)) {
                match skill_desc.skill_category {
                    1 => {
                        profession_exp.add_assign(es.quantity as i64);
                    }
                    2 => {
                        adventure_exp.add_assign(es.quantity as i64);
                    }
                    _ => {}
                }
            };

            if let Some(board) = self
                .global_app_state
                .ranking_system
                .skill_leaderboard(i64::from(es.skill_id))
            {
                board.update(id, i64::from(es.quantity));
            }

            let model = ::entity::experience_state::Model {
                entity_id: id,
                skill_id: es.skill_id,
                experience: es.quantity,
                region: database_name,
            };

            self.pending.upsert(model);
        });

        let current_score = self
            .global_app_state
            .ranking_system
            .global_leaderboard
            .get_value(&(new.entity_id as i64));
        let mut xp_per_hour = 0;
        if let Some(player_state) = self
            .global_app_state
            .player_state
            .get(&(new.entity_id as i64))
            && player_state.time_signed_in >= 3600
        {
            xp_per_hour = total_exp / (player_state.time_signed_in as i64 / 3600);
        }
        self.global_app_state
            .ranking_system
            .xp_per_hour
            .update(new.entity_id as i64, xp_per_hour);
        self.global_app_state
            .ranking_system
            .profession_leaderboard
            .update(new.entity_id as i64, profession_exp);
        self.global_app_state
            .ranking_system
            .adventure_leaderboard
            .update(new.entity_id as i64, adventure_exp);
        self.global_app_state
            .ranking_system
            .level_leaderboard
            .update(id, total_level);

        if let Some(current_score) = current_score {
            let current_known_xp = current_score;

            if current_known_xp < total_exp {
                self.global_app_state
                    .ranking_system
                    .global_leaderboard
                    .update(new.entity_id as i64, total_exp);
            }
        } else {
            self.global_app_state
                .ranking_system
                .global_leaderboard
                .update(new.entity_id as i64, total_exp);
        }
    }

    async fn handle_update(
        &mut self,
        new: ExperienceState,
        database_name: entity::shared::Region,
        old: ExperienceState,
    ) {
        // if new.entity_id == 1224979098660016778 {
        //     if let Some(Event::Reducer(event)) = &event {
        //         match last_time {
        //             Some(time) => {
        //                 let current_time = DateTime::from_timestamp_micros(event.timestamp.to_micros_since_unix_epoch()).unwrap();
        //
        //                 let diff = current_time.timestamp_millis() - time.timestamp_millis();
        //
        //                 tracing::error!("Diff {:?} Off {} Off % {} ", diff, diff - 1170, (diff as f64 / 1170f64 * 100f64) - 100f64);
        //
        //                 last_time = Some(current_time);
        //             }
        //             None => {
        //                 tracing::error!("AA {:?}", event.timestamp);
        //                 last_time = Some(DateTime::from_timestamp_micros(event.timestamp.to_micros_since_unix_epoch()).unwrap());
        //             }
        //         }
        //
        //     }
        // }

        let id = new.entity_id as i64;
        let mut new_total_exp = 0;
        let ranking = &self.global_app_state.ranking_system;
        let player = std::cell::OnceCell::new();
        // Resolve only if this event actually updates a board. Every board in
        // this RankingSystem shares the registry, while ranks remain fresh reads.
        let resolve_player =
            || player.get_or_init(|| ranking.global_leaderboard.resolve_player(id));
        let debug_ranks = tracing::enabled!(tracing::Level::DEBUG);
        let mut new_total_level = 0;
        let mut new_profession_exp = 0;
        let mut new_adventure_exp = 0;
        let calculate_skill = |es: &ExperienceStack| SkillCalculation {
            level: experience_to_level(es.quantity as i64),
            category: self
                .global_app_state
                .skill_desc
                .get(&(es.skill_id as i64))
                .map(|skill| skill.skill_category),
        };

        let mut old_total_exp = 0;
        let mut old_total_level = 0;
        let mut old_profession_exp = 0;
        let mut old_adventure_exp = 0;
        // One combined pass for aligned stacks, including unequal-length tails.
        // Only mismatched skill IDs need a lookup/recalculation; equal lengths
        // alone do not imply matching order or membership. Notifications retain
        // old-stack order, and each new stack contributes to totals exactly once.
        for index in 0..old.experience_stacks.len().max(new.experience_stacks.len()) {
            let indexed_new = new.experience_stacks.get(index);
            let indexed_calculation = indexed_new.map(|es| {
                let calculation = calculate_skill(es);
                new_total_exp.add_assign(es.quantity as i64);
                new_total_level.add_assign(calculation.level);
                match calculation.category {
                    Some(1) => new_profession_exp.add_assign(es.quantity as i64),
                    Some(2) => new_adventure_exp.add_assign(es.quantity as i64),
                    _ => {}
                }
                calculation
            });
            let Some(es) = old.experience_stacks.get(index) else {
                continue;
            };
            old_total_exp.add_assign(es.quantity as i64);
            let (new_skill, calculation) =
                if indexed_new.is_some_and(|skill| skill.skill_id == es.skill_id) {
                    (indexed_new, indexed_calculation)
                } else {
                    let matched = new
                        .experience_stacks
                        .iter()
                        .find(|skill| skill.skill_id == es.skill_id);
                    (matched, matched.map(&calculate_skill))
                };
            let old_level = match (new_skill, calculation) {
                (Some(new_skill), Some(calculation)) if new_skill.quantity == es.quantity => {
                    calculation.level
                }
                _ => experience_to_level(es.quantity as i64),
            };
            old_total_level.add_assign(old_level);
            let category = match calculation {
                Some(calculation) => calculation.category,
                None => self
                    .global_app_state
                    .skill_desc
                    .get(&(es.skill_id as i64))
                    .map(|skill| skill.skill_category),
            };
            if let Some(category) = category {
                match category {
                    1 => {
                        old_profession_exp.add_assign(es.quantity as i64);
                    }
                    2 => {
                        old_adventure_exp.add_assign(es.quantity as i64);
                    }
                    _ => {}
                }
            };

            if let Some(new_skill) = new_skill {
                let new_level = calculation.expect("matched skill has a calculation").level;
                let skill = if old_level != new_level || new_skill.quantity > es.quantity {
                    self.global_app_state
                        .skill_desc
                        .get(&(es.skill_id as i64))
                        .map(|skill| (skill.name.clone(), skill.skill_category))
                } else {
                    None
                };
                if old_level != new_level
                    && let Some((skill_name, _)) = &skill
                {
                    let _ = self
                        .global_app_state
                        .tx
                        .send(OutboundWebSocketMessages::Level {
                            level: new_level as u64,
                            skill_name: skill_name.clone(),
                            user_id: id,
                        });
                }

                if new_skill.quantity > es.quantity {
                    let model = ::entity::experience_state::Model {
                        entity_id: id,
                        skill_id: es.skill_id,
                        experience: new_skill.quantity,
                        region: database_name,
                    };

                    self.pending.upsert(model);

                    if let Some((skill_name, skill_category)) = skill
                        && skill_category != 0
                    {
                        let mut prev_rank = None;
                        let post_rank;
                        if let Some(skill_leaderboard) = self
                            .global_app_state
                            .ranking_system
                            .skill_leaderboard(es.skill_id as i64)
                        {
                            if debug_ranks {
                                prev_rank = skill_leaderboard.get_rank_resolved(resolve_player());
                            }
                            skill_leaderboard
                                .update_resolved(resolve_player(), new_skill.quantity as i64);
                            post_rank = skill_leaderboard.get_rank_resolved(resolve_player());
                        } else {
                            let skill_leaderboard = self
                                .global_app_state
                                .ranking_system
                                .ensure_skill_leaderboard(es.skill_id as i64);
                            skill_leaderboard
                                .update_resolved(resolve_player(), new_skill.quantity as i64);
                            post_rank = skill_leaderboard.get_rank_resolved(resolve_player());
                        }

                        match (prev_rank, post_rank) {
                            (Some(prev_rank), Some(post_rank)) => {
                                if prev_rank != post_rank {
                                    tracing::debug!(
                                        "Skill EXP rank {skill_name} changed {prev_rank} to {post_rank}"
                                    );
                                }
                            }
                            (Some(prev_rank), None) => {
                                tracing::debug!(
                                    "Skill EXP rank {skill_name} changed {prev_rank} to no post_rank"
                                );
                            }
                            (None, Some(post_rank)) => {
                                tracing::debug!(
                                    "Skill EXP rank {skill_name} changed no prev_rank to {post_rank}"
                                );
                            }
                            (None, None) => {
                                tracing::debug!(
                                    user_id = id,
                                    skill_id = es.skill_id,
                                    "Skill EXP rank not published yet"
                                );
                            }
                        }
                        let _ =
                            self.global_app_state
                                .tx
                                .send(OutboundWebSocketMessages::Experience {
                                    level: new_level as u64,
                                    experience: new_skill.quantity as u64,
                                    rank: post_rank.unwrap_or(0) as u64,
                                    skill_name,
                                    user_id: id,
                                });
                    }
                }
            }
        }

        if old_total_level != new_total_level {
            let prev_rank = if debug_ranks {
                self.global_app_state
                    .ranking_system
                    .level_leaderboard
                    .get_rank_resolved(resolve_player())
            } else {
                None
            };
            self.global_app_state
                .ranking_system
                .level_leaderboard
                .update_resolved(resolve_player(), new_total_level as i64);
            let post_rank = if debug_ranks {
                self.global_app_state
                    .ranking_system
                    .level_leaderboard
                    .get_rank_resolved(resolve_player())
            } else {
                None
            };

            match (prev_rank, post_rank) {
                (Some(prev_rank), Some(post_rank)) => {
                    if prev_rank != post_rank {
                        tracing::debug!("Level rank changed {prev_rank} to {post_rank}")
                    }
                }
                (Some(prev_rank), None) => {
                    tracing::debug!("Level rank changed {prev_rank} to no post_rank")
                }
                (None, Some(post_rank)) => {
                    tracing::debug!("Level rank changed no prev_rank to {post_rank}")
                }
                (None, None) => {
                    tracing::debug!(user_id = id, "Level rank not published yet")
                }
            }
        }

        if old_total_exp != new_total_exp {
            let prev_rank = if debug_ranks {
                self.global_app_state
                    .ranking_system
                    .global_leaderboard
                    .get_rank_resolved(resolve_player())
            } else {
                None
            };
            self.global_app_state
                .ranking_system
                .global_leaderboard
                .update_resolved(resolve_player(), new_total_exp);
            let mut xp_per_hour = 0;
            if let Some(player_state) = self
                .global_app_state
                .player_state
                .get(&(new.entity_id as i64))
                && player_state.time_signed_in >= 3600
            {
                xp_per_hour = new_total_exp / (player_state.time_signed_in as i64 / 3600);
            }
            self.global_app_state
                .ranking_system
                .xp_per_hour
                .update_resolved(resolve_player(), xp_per_hour);

            let post_rank = self
                .global_app_state
                .ranking_system
                .global_leaderboard
                .get_rank_resolved(resolve_player());

            match (prev_rank, post_rank) {
                (Some(prev_rank), Some(post_rank)) => {
                    if prev_rank != post_rank {
                        tracing::debug!("Total EXP rank changed {prev_rank} to {post_rank}")
                    }
                }
                (Some(prev_rank), None) => {
                    tracing::debug!("Total EXP rank changed {prev_rank} to no post_rank")
                }
                (None, Some(post_rank)) => {
                    tracing::debug!("Total EXP rank changed no prev_rank to {post_rank}")
                }
                (None, None) => {
                    tracing::debug!(user_id = id, "Total EXP rank not published yet")
                }
            }

            let _ = self
                .global_app_state
                .tx
                .send(OutboundWebSocketMessages::TotalExperience {
                    experience: new_total_exp as u64,
                    user_id: id,
                    experience_per_hour: xp_per_hour as u64,
                    rank: post_rank.unwrap_or(0) as u64,
                });
        }

        if old_adventure_exp != new_adventure_exp {
            self.global_app_state
                .ranking_system
                .adventure_leaderboard
                .update_resolved(resolve_player(), new_adventure_exp);

            let post_rank = self
                .global_app_state
                .ranking_system
                .adventure_leaderboard
                .get_rank_resolved(resolve_player());

            let _ = self.global_app_state.tx.send(
                OutboundWebSocketMessages::TotalAdventureExperience {
                    experience: new_adventure_exp as u64,
                    user_id: id,
                    rank: post_rank.unwrap_or(0) as u64,
                },
            );
        }

        if old_profession_exp != new_profession_exp {
            self.global_app_state
                .ranking_system
                .profession_leaderboard
                .update_resolved(resolve_player(), new_profession_exp);

            let post_rank = self
                .global_app_state
                .ranking_system
                .profession_leaderboard
                .get_rank_resolved(resolve_player());

            let _ = self.global_app_state.tx.send(
                OutboundWebSocketMessages::TotalProfessionExperience {
                    experience: new_profession_exp as u64,
                    user_id: id,
                    rank: post_rank.unwrap_or(0) as u64,
                },
            );
        }
    }

    async fn handle_remove(
        &mut self,
        delete: ExperienceState,
        _database_name: entity::shared::Region,
    ) {
        let id = delete.entity_id as i64;
        for es in delete.experience_stacks {
            self.pending.delete((id, es.skill_id));
        }
    }

    async fn flush_messages(&mut self) {
        if self.pending.upserts.is_empty() {
            return;
        }

        let messages = self
            .pending
            .upserts
            .drain()
            .map(|(_, model)| model.into_active_model())
            .collect();
        self.queue_upserts(messages).await;
    }

    async fn flush_deletes(&mut self) {
        if self.pending.deletes.is_empty() {
            return;
        }

        tracing::debug!("ExperienceState::Remove");
        let messages_delete = self.pending.deletes.drain().collect();
        self.queue_deletes(messages_delete).await;
    }
}

impl BatchedWorker for ExperienceStateWorker {
    type Entity = ExperienceState;

    fn rx(&mut self) -> &mut UnboundedReceiver<SpacetimeUpdateMessages<Self::Entity>> {
        &mut self.rx
    }

    fn tx(&self) -> UnboundedSender<SpacetimeUpdateMessages<Self::Entity>> {
        self.tx.clone()
    }

    fn worker_name(&self) -> &'static str {
        "experience_state"
    }

    fn batch_delay(&self) -> Duration {
        self.time_limit
    }

    fn should_flush(&self) -> bool {
        self.pending.upserts.len() >= self.batch_size
            || self.pending.deletes.len() >= self.batch_size
    }

    fn is_idle(&self) -> bool {
        self.pending.upserts.is_empty() && self.pending.deletes.is_empty()
    }

    fn reset_batch(&mut self) {}

    async fn handle_message(&mut self, msg: SpacetimeUpdateMessages<ExperienceState>) {
        self.process_message(msg).await;
    }

    async fn flush(&mut self) {
        self.flush_messages().await;
        self.flush_deletes().await;
    }
}

async fn insert_multiple_experience_state(
    global_app_state: &AppState,
    on_conflict: &OnConflict,
    messages: Vec<::entity::experience_state::ActiveModel>,
) {
    let result = ::entity::experience_state::Entity::insert_many(messages)
        .on_conflict(on_conflict.clone())
        .exec(&global_app_state.conn)
        .await;

    if let Err(error) = result {
        tracing::error!(
            error = error.to_string(),
            "Error while saving experience_state"
        );
    }
}
