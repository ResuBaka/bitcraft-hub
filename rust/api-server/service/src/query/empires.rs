use super::{Query, watchtowers};
use entity::empire::views::{
    EmpireDetail, EmpireMember, EmpireNews, EmpireResources, EmpireSettlement, EmpireSummary,
};
use sea_orm::{DbBackend, DbConn, DbErr, FromQueryResult, Statement};

// Regional names can be missing even when the global settlement and claim ID exist.
const SUMMARY: &str = "
    SELECT e.entity_id::text AS id, e.name, e.owner_type = 1 AS npc,
           to_hex(c1.color_argb & 16777215) AS color, to_hex(c2.color_argb & 16777215) AS accent,
           capital.claim_entity_id::text AS capital_id, c.name AS capital_name,
           (SELECT count(*)::int FROM empire_player_data_state WHERE empire_entity_id=e.entity_id) AS members,
           (SELECT count(*)::int FROM empire_settlement_state WHERE empire_entity_id=e.entity_id) AS settlements,
           (SELECT count(*)::int FROM empire_chunk_state WHERE empire_entity_id=e.entity_id) AS chunks,
           (SELECT count(*)::int FROM empire_node_state WHERE empire_entity_id=e.entity_id) AS towers
    FROM empire_state e
    LEFT JOIN empire_emblem_state em ON em.entity_id=e.entity_id
    LEFT JOIN empire_color_desc c1 ON c1.id=em.color_1_id
    LEFT JOIN empire_color_desc c2 ON c2.id=em.color_2_id
    LEFT JOIN empire_settlement_state capital ON capital.building_entity_id=e.capital_building_entity_id
    LEFT JOIN claim_state c ON c.entity_id=capital.claim_entity_id";

impl Query {
    pub async fn empire_directory(db: &DbConn) -> Result<Vec<EmpireSummary>, DbErr> {
        EmpireSummary::find_by_statement(Statement::from_string(
            DbBackend::Postgres,
            format!("{SUMMARY} ORDER BY chunks DESC, lower(e.name), e.entity_id"),
        ))
        .all(db)
        .await
    }

    pub async fn empire_detail(db: &DbConn, id: i64) -> Result<Option<EmpireDetail>, DbErr> {
        let stmt =
            |sql: &str| Statement::from_sql_and_values(DbBackend::Postgres, sql, [id.into()]);
        let Some(empire) =
            EmpireSummary::find_by_statement(stmt(&format!("{SUMMARY} WHERE e.entity_id=$1")))
                .one(db)
                .await?
        else {
            return Ok(None);
        };
        let (resources, members, settlements, news, towers, former_towers) = tokio::try_join!(
            EmpireResources::find_by_statement(stmt("
                SELECT e.shard_treasury, e.empire_currency_treasury AS currency_treasury, d.directive_message AS directive,
                    (SELECT count(*)::int FROM empire_node_state WHERE empire_entity_id=$1 AND active) AS active_towers,
                    (SELECT count(*)::int FROM empire_foundry_state WHERE empire_entity_id=$1) AS foundries,
                    (SELECT count(*)::int FROM empire_node_siege_state WHERE empire_entity_id=$1 AND active) AS active_sieges
                FROM empire_state e LEFT JOIN empire_directive_state d ON d.entity_id=e.entity_id WHERE e.entity_id=$1"
            )).one(db),
            EmpireMember::find_by_statement(stmt("
                SELECT p.entity_id::text AS id, u.username, p.rank, coalesce(r.title, rd.title) AS title,
                    p.donated_shards, p.donated_empire_currency AS donated_currency
                FROM empire_player_data_state p LEFT JOIN player_username_state u ON u.entity_id=p.entity_id
                LEFT JOIN empire_rank_state r ON r.empire_entity_id=p.empire_entity_id AND r.rank=p.rank
                LEFT JOIN empire_rank_desc rd ON rd.rank=p.rank WHERE p.empire_entity_id=$1
                ORDER BY p.rank, lower(u.username), p.entity_id"
            )).all(db),
            EmpireSettlement::find_by_statement(stmt("
                SELECT s.claim_entity_id::text AS id, c.name, c.region, s.x, s.z FROM empire_settlement_state s
                LEFT JOIN claim_state c ON c.entity_id=s.claim_entity_id WHERE s.empire_entity_id=$1
                ORDER BY lower(c.name), s.claim_entity_id"
            )).all(db),
            EmpireNews::find_by_statement(stmt("
                SELECT n.entity_id::text AS id, n.timestamp, d.text AS template, n.text_replacement AS replacements
                FROM empire_notification_state n LEFT JOIN empire_notification_desc d USING (notification_type)
                WHERE n.empire_entity_id=$1 ORDER BY n.timestamp DESC, n.entity_id DESC LIMIT 15"
            )).all(db),
            watchtowers::owned(db, id),
            watchtowers::former(db, id),
        )?;
        Ok(resources.map(|resources| EmpireDetail {
            empire,
            resources,
            members,
            settlements,
            news,
            towers,
            former_towers,
        }))
    }
}
