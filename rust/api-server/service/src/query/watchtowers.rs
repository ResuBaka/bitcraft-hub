use super::Query;
use entity::empire::views::{
    FormerWatchtower, Watchtower, WatchtowerDetail, WatchtowerObservation, WatchtowerReport,
    WatchtowerSiege,
};
use sea_orm::{DatabaseConnection, DbBackend, DbErr, FromQueryResult, Statement};

const TOWERS: &str = "
    SELECT n.entity_id::text AS id, b.nickname, n.empire_entity_id::text AS empire_id,
           e.name AS empire_name, n.x, n.z, n.dimension, building.region, n.active, n.energy, n.upkeep,
           (SELECT empire_decay_tick_millis FROM empire_decay_parameters WHERE version=0) AS decay_tick_millis,
           territory.assigned_chunks, territory.controlled_chunks,
           EXISTS (SELECT FROM empire_node_siege_state WHERE building_entity_id=n.entity_id AND active) AS under_siege,
           EXISTS (SELECT FROM empire_node_siege_state WHERE building_entity_id=n.entity_id AND NOT active) AS marked
    FROM empire_node_state n
    LEFT JOIN empire_state e ON e.entity_id=n.empire_entity_id
    LEFT JOIN building_nickname_state b ON b.entity_id=n.entity_id
    LEFT JOIN building_state building ON building.entity_id=n.entity_id
    CROSS JOIN LATERAL (
        SELECT count(*)::int AS assigned_chunks,
               count(*) FILTER (WHERE c.empire_entity_id<>0 AND c.empire_entity_id=n.empire_entity_id)::int AS controlled_chunks
        FROM empire_chunk_state c WHERE c.watchtower_entity_id=n.entity_id
    ) territory";

fn stmt(sql: &str, id: i64) -> Statement {
    Statement::from_sql_and_values(DbBackend::Postgres, sql, [id.into()])
}

pub(super) async fn owned(db: &DatabaseConnection, empire: i64) -> Result<Vec<Watchtower>, DbErr> {
    Watchtower::find_by_statement(stmt(
        &format!(
            "{TOWERS} WHERE n.empire_entity_id=$1
        ORDER BY under_siege DESC, marked DESC, n.active DESC, n.x, n.z, n.entity_id"
        ),
        empire,
    ))
    .all(db)
    .await
}

pub(super) async fn former(
    db: &DatabaseConnection,
    empire: i64,
) -> Result<Vec<FormerWatchtower>, DbErr> {
    FormerWatchtower::find_by_statement(stmt("
        SELECT DISTINCT ON (h.tower_id) h.tower_id::text AS id, b.nickname,
            (h.before_state->>'x')::int AS x, (h.before_state->>'z')::int AS z
        FROM watchtower_history h
        LEFT JOIN building_nickname_state b ON b.entity_id=h.tower_id
        WHERE source='tower' AND before_state->>'empire_entity_id'=($1::bigint)::text
          AND before_state->>'empire_entity_id' IS DISTINCT FROM after_state->>'empire_entity_id'
          AND NOT EXISTS (SELECT FROM empire_node_state n WHERE n.entity_id=h.tower_id AND n.empire_entity_id=$1)
        ORDER BY h.tower_id, h.id DESC", empire)).all(db).await
}

// BitCraft's EmpireNotificationState::coord_to_string scales *axial* small
// hex coordinates by 1/3, rounds, then converts back to large offset tiles.
// Reports have no building ID: keep them separate from exact-ID observations.
const LOCATION_REPORTS: &str = "
    WITH last_node AS (
        SELECT coalesce(after_state, before_state) AS node FROM watchtower_history
        WHERE tower_id=$1 AND source='tower' ORDER BY id DESC LIMIT 1
    ), loc AS (
        SELECT (node->>'x')::int AS x, (node->>'z')::int AS z FROM last_node
        WHERE (node->>'dimension')::bigint=1
    ), tile AS (
        SELECT round(z/3.0)::int AS north, round((x-z/2)/3.0)::int AS axial_x FROM loc
    )
    SELECT n.entity_id::text AS id, n.timestamp, n.notification_type,
           n.empire_entity_id::text AS empire_id, e.name AS empire_name
    FROM tile JOIN empire_notification_state n ON n.text_replacement[array_length(n.text_replacement,1)]
        = format('N:{0}, E:{1}|~%s|~%s', north, axial_x+north/2)
    LEFT JOIN empire_state e ON e.entity_id=n.empire_entity_id
    WHERE n.notification_type IN (2,3,4,5,6,7,8,10)
    ORDER BY n.timestamp DESC, n.entity_id DESC LIMIT 25";

const SIEGES: &str = "
    SELECT s.entity_id::text AS id, s.empire_entity_id::text AS empire_id, e.name AS empire_name,
           s.energy, s.active, s.start_timestamp
    FROM empire_node_siege_state s LEFT JOIN empire_state e ON e.entity_id=s.empire_entity_id
    WHERE s.building_entity_id=$1 ORDER BY s.active DESC, s.entity_id";

const HISTORY: &str = "
    SELECT h.id::text AS id, h.observed_at, h.source, h.event,
           h.before_state || jsonb_build_object('empire_id', h.before_state->>'empire_entity_id', 'empire_name', p.name) AS previous,
           h.after_state || jsonb_build_object('empire_id', h.after_state->>'empire_entity_id', 'empire_name', c.name) AS current
    FROM watchtower_history h
    LEFT JOIN empire_state p ON p.entity_id=(h.before_state->>'empire_entity_id')::bigint
    LEFT JOIN empire_state c ON c.entity_id=(h.after_state->>'empire_entity_id')::bigint
    WHERE h.tower_id=$1 AND h.id < coalesce($2::bigint, 9223372036854775807)
    ORDER BY h.id DESC LIMIT $3";

impl Query {
    pub async fn watchtower_detail(
        db: &DatabaseConnection,
        id: i64,
        before: Option<i64>,
    ) -> Result<Option<WatchtowerDetail>, DbErr> {
        const PAGE: usize = 30;
        let (tower, sieges, mut history, reports) = tokio::try_join!(
            Watchtower::find_by_statement(stmt(&format!("{TOWERS} WHERE n.entity_id=$1"), id))
                .one(db),
            WatchtowerSiege::find_by_statement(stmt(SIEGES, id)).all(db),
            WatchtowerObservation::find_by_statement(Statement::from_sql_and_values(
                DbBackend::Postgres,
                HISTORY,
                [id.into(), before.into(), ((PAGE + 1) as i64).into()],
            ))
            .all(db),
            WatchtowerReport::find_by_statement(stmt(LOCATION_REPORTS, id)).all(db),
        )?;
        if tower.is_none() && history.is_empty() {
            // On later pages the cursor may simply be past the oldest event.
            #[derive(FromQueryResult)]
            struct Known {
                present: bool,
            }
            if before.is_none()
                || !Known::find_by_statement(stmt(
                    "SELECT EXISTS(SELECT FROM watchtower_history WHERE tower_id=$1) AS present",
                    id,
                ))
                .one(db)
                .await?
                .is_some_and(|k| k.present)
            {
                return Ok(None);
            }
        }
        let next_before = if history.len() > PAGE {
            history.truncate(PAGE);
            history.last().map(|row| row.id.clone())
        } else {
            None
        };
        Ok(Some(WatchtowerDetail {
            tower,
            sieges,
            history,
            reports,
            next_before,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::MigratorTrait;
    use sea_orm::{ConnectOptions, ConnectionTrait, Database};

    #[tokio::test]
    #[ignore = "requires EMPIRE_TEST_DATABASE_URL; uses an isolated PostgreSQL schema"]
    async fn tower_history_survives_transfer_removal_and_reconciliation() -> anyhow::Result<()> {
        let schema = format!("tower_test_{}", rand::random::<u64>());
        let mut options = ConnectOptions::new(std::env::var("EMPIRE_TEST_DATABASE_URL")?);
        options.max_connections(3).set_schema_search_path(&schema);
        let db = Database::connect(options).await?;
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await?;
        let manager = migration::SchemaManager::new(&db);
        let migrations = migration::Migrator::migrations();
        let empire = migrations
            .iter()
            .find(|m| m.name() == "m20261124_000001_empire")
            .unwrap();
        let history = migrations
            .iter()
            .find(|m| m.name() == "m20261124_000002_watchtower_history")
            .unwrap();
        empire.up(&manager).await?;
        migrations
            .iter()
            .find(|m| m.name() == "m20261124_000003_empire_decay_parameters")
            .unwrap()
            .up(&manager)
            .await?;
        db.execute_unprepared(
            "
            CREATE TABLE building_state (entity_id bigint PRIMARY KEY, region smallint);
            CREATE TABLE building_nickname_state (entity_id bigint PRIMARY KEY, nickname text);
            INSERT INTO empire_decay_parameters VALUES (0, 60000), (1, 123);
            INSERT INTO empire_node_state VALUES (9007199254740993,1,9,150,true,5,11,9,1);
            INSERT INTO empire_notification_state VALUES
                (1,1,10,ARRAY['N:{0}, E:{1}|~3|~3'],123),
                (2,1,10,ARRAY['N:{0}, E:{1}|~3|~4'],124);",
        )
        .await?;
        history.up(&manager).await?;
        let id = 9007199254740993;
        db.execute_unprepared(
            "
            UPDATE empire_node_state SET energy=energy;
            INSERT INTO empire_node_siege_state VALUES (44,9007199254740993,2,0,false,NULL);
            UPDATE empire_node_siege_state SET energy=60, active=true, start_timestamp=now();",
        )
        .await?;
        let view = Query::watchtower_detail(&db, id, None).await?.unwrap();
        let tower = view.tower.unwrap();
        assert!(tower.under_siege);
        assert_eq!(tower.decay_tick_millis, Some(60_000));
        assert_eq!(view.history.len(), 3);
        assert_eq!(view.history[2].event, "baseline");
        assert!(!view.history[0].previous.as_ref().unwrap().active);
        assert!(view.history[0].current.as_ref().unwrap().active);
        assert_eq!(view.reports.len(), 1);
        assert_eq!(view.reports[0].id, "1");
        db.execute_unprepared("
            DELETE FROM empire_node_siege_state;
            UPDATE empire_node_state SET empire_entity_id=2;
            DO $$ BEGIN FOR i IN 1..31 LOOP UPDATE empire_node_state SET energy=energy+1; END LOOP; END $$;
            DELETE FROM empire_node_state;").await?;
        assert_eq!(former(&db, 1).await?[0].id, id.to_string());
        let newest = Query::watchtower_detail(&db, id, None).await?.unwrap();
        assert!(newest.tower.is_none() && newest.sieges.is_empty());
        assert_eq!(newest.history.len(), 30);
        assert_eq!(newest.history[0].event, "removed");
        let cursor = newest.next_before.unwrap().parse()?;
        let older = Query::watchtower_detail(&db, id, Some(cursor))
            .await?
            .unwrap();
        assert_eq!(older.history.len(), 7);
        assert!(
            older
                .history
                .iter()
                .all(|r| r.id.parse::<i64>().unwrap() < cursor)
        );
        assert!(older.next_before.is_none());
        assert!(Query::watchtower_detail(&db, 42, None).await?.is_none());
        history.down(&manager).await?;
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
            .await?;
        Ok(())
    }
}
