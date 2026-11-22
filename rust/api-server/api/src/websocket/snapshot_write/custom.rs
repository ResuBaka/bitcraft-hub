use super::{DbBackend, DbErr, NonZeroUsize, Statement, Writer};
use entity::{extraction_recipe_desc as recipe, player_state};

pub(crate) fn players(rows: &[player_state::Model]) -> Statement {
    arrays!(rows, "INSERT INTO player_state (entity_id, time_played, session_start_timestamp, time_signed_in,
            sign_in_timestamp, signed_in, teleport_location, traveler_tasks_expiration, region)
        SELECT * FROM unnest($1::bigint[], $2::integer[], $3::integer[], $4::integer[], $5::integer[],
            $6::boolean[], $7::jsonb[], $8::integer[], $9::smallint[])
        ON CONFLICT (entity_id) DO UPDATE SET time_played = EXCLUDED.time_played,
            session_start_timestamp = EXCLUDED.session_start_timestamp, time_signed_in = EXCLUDED.time_signed_in,
            sign_in_timestamp = EXCLUDED.sign_in_timestamp, signed_in = EXCLUDED.signed_in,
            teleport_location = EXCLUDED.teleport_location, traveler_tasks_expiration = EXCLUDED.traveler_tasks_expiration,
            region = EXCLUDED.region",
        |r| r.entity_id, r.time_played, r.session_start_timestamp, r.time_signed_in, r.sign_in_timestamp,
        r.signed_in, serde_json::json!(&r.teleport_location), r.traveler_tasks_expiration, r.region)
}

pub(crate) fn extraction_writer() -> Writer<'static, recipe::Entity> {
    Writer::ByteLimited {
        statement: extraction_recipes,
        // The JSON elements consist only of fixed field names, enums and numbers.
        estimate: |r| {
            256usize
                .saturating_add(192usize.saturating_mul(r.extracted_item_stacks.len()))
                .saturating_add(96usize.saturating_mul(r.tool_requirements.len()))
                .saturating_add(
                    80usize.saturating_mul(r.experience_per_progress.as_ref().map_or(0, Vec::len)),
                )
        },
        limit: NonZeroUsize::new(1024 * 1024).unwrap(),
    }
}

fn extraction_recipes(rows: &[recipe::Model]) -> Result<Statement, DbErr> {
    // Keep the legacy Value encoding (notably f32 -> JSON number precision),
    // but retain only compact text for the batch, not all the nested trees.
    // For optional XP, JSON `null` denotes SQL NULL, not [].
    fn json_text<T: serde::Serialize>(
        values: impl Iterator<Item = T>,
    ) -> Result<sea_orm::Value, DbErr> {
        values
            .map(|v| serde_json::to_value(v).and_then(|json| serde_json::to_string(&json)))
            .collect::<Result<Vec<_>, _>>()
            .map(Into::into)
            .map_err(|err| DbErr::Json(err.to_string()))
    }
    Ok(Statement::from_sql_and_values(DbBackend::Postgres, "INSERT INTO extraction_recipe_desc (id, resource_id, extracted_item_stacks, tool_requirements,
            allow_use_hands, time_requirement, stamina_requirement, cargo_id, experience_per_progress)
        SELECT id, resource, stacks::json, tools::json, hands, duration, stamina, CASE WHEN has_cargo THEN cargo END,
            NULLIF(xp, 'null')::jsonb
        FROM unnest($1::integer[], $2::integer[], $3::text[], $4::text[], $5::boolean[], $6::real[],
            $7::real[], $8::integer[], $9::boolean[], $10::text[])
            AS b(id, resource, stacks, tools, hands, duration, stamina, cargo, has_cargo, xp)
        ON CONFLICT (id) DO UPDATE SET resource_id = EXCLUDED.resource_id, extracted_item_stacks = EXCLUDED.extracted_item_stacks,
            tool_requirements = EXCLUDED.tool_requirements, allow_use_hands = EXCLUDED.allow_use_hands,
            time_requirement = EXCLUDED.time_requirement, stamina_requirement = EXCLUDED.stamina_requirement,
            cargo_id = EXCLUDED.cargo_id, experience_per_progress = EXCLUDED.experience_per_progress",
        [
            rows.iter().map(|r| r.id).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.resource_id).collect::<Vec<_>>().into(),
            json_text(rows.iter().map(|r| &r.extracted_item_stacks))?,
            json_text(rows.iter().map(|r| &r.tool_requirements))?,
            rows.iter().map(|r| r.allow_use_hands).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.time_requirement).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.stamina_requirement).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.cargo_id.unwrap_or_default()).collect::<Vec<_>>().into(),
            rows.iter().map(|r| r.cargo_id.is_some()).collect::<Vec<_>>().into(),
            json_text(rows.iter().map(|r| &r.experience_per_progress))?,
        ]))
}
