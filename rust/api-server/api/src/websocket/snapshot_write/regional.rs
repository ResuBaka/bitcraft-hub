use super::{DbBackend, NonZeroUsize, Statement, Writer};
use entity::{
    building_state, deployable_state, experience_state, progressive_action_state,
    traveler_task_state,
};

pub(crate) fn experience(rows: &[experience_state::Model]) -> Statement {
    arrays!(rows, "INSERT INTO experience_state (entity_id, skill_id, experience, region)
        SELECT * FROM unnest($1::bigint[], $2::integer[], $3::integer[], $4::smallint[])
        ON CONFLICT (entity_id, skill_id) DO UPDATE SET experience = EXCLUDED.experience, region = EXCLUDED.region",
        |r| r.entity_id, r.skill_id, r.experience, r.region)
}

pub(crate) fn traveler_tasks(rows: &[traveler_task_state::Model]) -> Statement {
    arrays!(rows, "INSERT INTO traveler_task_state (entity_id, player_entity_id, traveler_id, task_id, completed, region)
        SELECT * FROM unnest($1::bigint[], $2::bigint[], $3::integer[], $4::integer[], $5::boolean[], $6::smallint[])
        ON CONFLICT (entity_id) DO UPDATE SET player_entity_id = EXCLUDED.player_entity_id,
            traveler_id = EXCLUDED.traveler_id, task_id = EXCLUDED.task_id, completed = EXCLUDED.completed,
            region = EXCLUDED.region",
        |r| r.entity_id, r.player_entity_id, r.traveler_id, r.task_id, r.completed, r.region)
}

pub(crate) fn buildings(rows: &[building_state::Model]) -> Statement {
    arrays!(rows, "INSERT INTO building_state (entity_id, claim_entity_id, direction_index, building_description_id,
            constructed_by_player_entity_id, region)
        SELECT * FROM unnest($1::bigint[], $2::bigint[], $3::integer[], $4::integer[], $5::bigint[], $6::smallint[])
        ON CONFLICT (entity_id) DO UPDATE SET claim_entity_id = EXCLUDED.claim_entity_id, direction_index = EXCLUDED.direction_index,
            building_description_id = EXCLUDED.building_description_id, constructed_by_player_entity_id = EXCLUDED.constructed_by_player_entity_id,
            region = EXCLUDED.region",
        |r| r.entity_id, r.claim_entity_id, r.direction_index, r.building_description_id, r.constructed_by_player_entity_id, r.region)
}

pub(crate) fn deployable_writer() -> Writer<'static, deployable_state::Entity> {
    Writer::ByteLimited {
        statement: |rows| Ok(deployables(rows)),
        // Scalar columns, bind framing, and the UTF-8 nickname payload.
        estimate: |row| 128usize.saturating_add(row.nickname.len()),
        limit: NonZeroUsize::new(1024 * 1024).unwrap(),
    }
}

fn deployables(rows: &[deployable_state::Model]) -> Statement {
    // As in the live writer, appearance_override_id is insert-only on conflict.
    arrays!(rows, "INSERT INTO deployable_state (entity_id, owner_id, claim_entity_id, direction,
            deployable_description_id, nickname, hidden, appearance_override_id, region)
        SELECT * FROM unnest($1::bigint[], $2::bigint[], $3::bigint[], $4::integer[], $5::integer[], $6::text[],
            $7::boolean[], $8::integer[], $9::smallint[])
        ON CONFLICT (entity_id) DO UPDATE SET owner_id = EXCLUDED.owner_id, claim_entity_id = EXCLUDED.claim_entity_id,
            direction = EXCLUDED.direction, deployable_description_id = EXCLUDED.deployable_description_id,
            nickname = EXCLUDED.nickname, hidden = EXCLUDED.hidden, region = EXCLUDED.region",
        |r| r.entity_id, r.owner_id, r.claim_entity_id, r.direction, r.deployable_description_id, r.nickname.clone(),
            r.hidden, r.appearance_override_id, r.region)
}

pub(crate) fn progressive_actions(rows: &[progressive_action_state::Model]) -> Statement {
    // lock_expiration is the existing serialized Timestamp object, not a SQL timestamp.
    arrays!(rows, "INSERT INTO progressive_action_state (entity_id, building_entity_id, function_type, progress,
            recipe_id, craft_count, last_crit_outcome, owner_entity_id, lock_expiration, preparation, region)
        SELECT * FROM unnest($1::bigint[], $2::bigint[], $3::integer[], $4::integer[], $5::integer[],
            $6::integer[], $7::integer[], $8::bigint[], $9::jsonb[], $10::boolean[], $11::smallint[])
        ON CONFLICT (entity_id) DO UPDATE SET building_entity_id = EXCLUDED.building_entity_id, function_type = EXCLUDED.function_type,
            progress = EXCLUDED.progress, recipe_id = EXCLUDED.recipe_id, craft_count = EXCLUDED.craft_count,
            last_crit_outcome = EXCLUDED.last_crit_outcome, owner_entity_id = EXCLUDED.owner_entity_id,
            lock_expiration = EXCLUDED.lock_expiration, preparation = EXCLUDED.preparation, region = EXCLUDED.region",
        |r| r.entity_id, r.building_entity_id, r.function_type, r.progress, r.recipe_id, r.craft_count,
            r.last_crit_outcome, r.owner_entity_id, serde_json::json!(&r.lock_expiration), r.preparation, r.region)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::initial_sync::{bigint_keys_filter, reconcile};
    use sea_orm::{DbErr, MockDatabase, MockExecResult, Value};
    use std::collections::BTreeMap;

    #[tokio::test]
    async fn deployable_subwrite_failure_stops_before_next_row_and_stale_deletes() {
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![BTreeMap::from([("0", Value::from(99i64))])]])
            .append_exec_results([MockExecResult::default()])
            .append_exec_errors([DbErr::Custom("second write failed".into())])
            .into_connection();
        let input = (1..=3).map(|id| deployable_state::Model {
            entity_id: id,
            owner_id: 1,
            claim_entity_id: 2,
            direction: 3,
            deployable_description_id: 4,
            nickname: if id == 2 {
                "猫".repeat(350_000)
            } else {
                "cart".into()
            },
            hidden: false,
            appearance_override_id: 5,
            region: 7,
        });
        let result = reconcile::<deployable_state::Entity, _, _, _, _, _>(
            &db,
            deployable_state::Column::Region,
            7,
            deployable_writer(),
            3000,
            input,
            |r| r.entity_id,
            |ids| bigint_keys_filter(deployable_state::Column::EntityId, ids),
            |_, _| {},
        )
        .await;
        assert_eq!(result, Err(DbErr::Custom("second write failed".into())));
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 3, "key read and two attempted upserts only");
        for (tx, id) in log[1..].iter().zip([1i64, 2]) {
            let stmt = &tx.statements()[0];
            assert!(stmt.sql.contains("INSERT INTO deployable_state"));
            assert_eq!(stmt.values.as_ref().unwrap().0[0], Value::from(vec![id]));
        }
    }
}
