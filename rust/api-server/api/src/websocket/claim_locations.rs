//! Cacheless subscriptions for claim tiles, their locations and interior claims.
use crate::claims::claim_location::ClaimLocationSubscriptions;
use crate::claims::map_details::{Building, BuildingKind};
use entity::shared::Region;
use game_module::module_bindings::*;
use spacetimedb_sdk::{Event, Table, TableWithPrimaryKey};
use std::sync::Arc;

pub(super) const MAP_BUILDING_TABLES: &[&str] = &["bank_state", "regional_bank_state", "marketplace_state", "waystone_state"];

fn listen_map_buildings(ctx: &DbConnection, region: Region, index: &Arc<ClaimLocationSubscriptions>) {
    macro_rules! listen {
        ($table:ident, $kind:expr) => {{
            let table = ctx.db.$table();
            let kind = $kind;
            let target = index.clone();
            table.on_initial_rows(move |_, rows| {
                target.set_map_buildings(region, kind, rows.iter().map(|row| (
                    row.building_entity_id,
                    Building::from_game(kind, row.claim_entity_id, &row.coordinates),
                )));
            });
            let target = index.clone();
            table.on_insert(move |ctx, row| {
                if !matches!(ctx.event, Event::SubscribeApplied) {
                    target.set_map_buildings(region, kind, [(
                        row.building_entity_id,
                        Building::from_game(kind, row.claim_entity_id, &row.coordinates),
                    )]);
                }
            });
            let target = index.clone();
            table.on_update(move |_, _, row| {
                target.set_map_buildings(region, kind, [(
                    row.building_entity_id,
                    Building::from_game(kind, row.claim_entity_id, &row.coordinates),
                )]);
            });
            let target = index.clone();
            table.on_delete(move |_, row| {
                target.set_map_buildings(region, kind, [(row.building_entity_id, None)]);
            });
        }};
    }
    listen!(bank_state, BuildingKind::Bank);
    listen!(regional_bank_state, BuildingKind::RegionalBank);
    listen!(marketplace_state, BuildingKind::Market);
    listen!(waystone_state, BuildingKind::Waystone);
}

pub(super) fn listen(ctx: &DbConnection, region: Region, index: Arc<ClaimLocationSubscriptions>) {
    listen_map_buildings(ctx, region, &index);
    let table = ctx.db.claim_tile_state();
    let target = index.clone();
    table.on_initial_rows(move |_, rows| target.claims_initial(region, rows.iter()));
    let target = index.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) { target.claim(region, row.entity_id, Some(row.claim_id)); }
    });
    let target = index.clone();
    table.on_update(move |_, _, row| target.claim(region, row.entity_id, Some(row.claim_id)));
    let target = index.clone();
    table.on_delete(move |_, row| target.claim(region, row.entity_id, None));

    let table = ctx.db.location_state();
    let target = index.clone();
    table.on_initial_rows(move |_, rows| target.locations_initial(region, rows.iter()));
    let target = index.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) { target.location(region, row.entity_id, Some((row.dimension, row.x, row.z))); }
    });
    let target = index.clone();
    table.on_update(move |_, _, row| target.location(region, row.entity_id, Some((row.dimension, row.x, row.z))));
    let target = index.clone();
    table.on_delete(move |_, row| target.location(region, row.entity_id, None));

    let table = ctx.db.dimension_description_state();
    let target = index.clone();
    table.on_initial_rows(move |_, rows| target.descriptions_initial(region, rows.iter()));
    let target = index.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) { target.description(region, row.dimension_id, Some(row.dimension_network_entity_id)); }
    });
    let target = index.clone();
    table.on_update(move |_, old, row| {
        if old.dimension_id != row.dimension_id { target.description(region, old.dimension_id, None); }
        target.description(region, row.dimension_id, Some(row.dimension_network_entity_id));
    });
    let target = index.clone();
    table.on_delete(move |_, row| target.description(region, row.dimension_id, None));

    let table = ctx.db.dimension_network_state();
    let target = index.clone();
    table.on_initial_rows(move |_, rows| target.networks_initial(region, rows.iter()));
    let target = index.clone();
    table.on_insert(move |ctx, row| {
        if !matches!(ctx.event, Event::SubscribeApplied) { target.network(region, row.entity_id, Some(row.claim_entity_id)); }
    });
    let target = index.clone();
    table.on_update(move |_, _, row| target.network(region, row.entity_id, Some(row.claim_entity_id)));
    table.on_delete(move |_, row| index.network(region, row.entity_id, None));
}
