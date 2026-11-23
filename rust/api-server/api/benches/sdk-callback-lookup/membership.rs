//! Untimed membership changes on the same replay connection. Run in both cache modes.
use game_module::module_bindings::*;
use spacetimedb_sdk::{
    __codegen::{
        self as sdk,
        __sats::bsatn,
        __ws::{
            common::{BsatnRowList, RowSizeHint},
            v2,
        },
    },
    Table, TableWithPrimaryKey,
};
use std::cell::RefCell;

thread_local! { static SEEN: RefCell<Vec<(i32, i32)>> = const { RefCell::new(Vec::new()) }; }

fn rows(x: Option<i32>) -> BsatnRowList {
    x.map_or_else(BsatnRowList::default, |x| {
        let row = LocationState {
            entity_id: 7,
            chunk_index: 0,
            x,
            z: 0,
            dimension: 1,
        };
        let bytes = bsatn::to_vec(&row).unwrap();
        BsatnRowList::new(
            RowSizeHint::FixedSize(bytes.len().try_into().unwrap()),
            bytes.into(),
        )
    })
}

fn main() {
    let c = sdk::DbContextImpl::<RemoteModule>::replay_context();
    let db: DbConnection = sdk::DbConnection::new(c.clone());
    let table = db.db.location_state();
    let apply = |old, new| {
        let update = v2::TransactionUpdate {
            query_sets: vec![v2::QuerySetUpdate {
                query_set_id: v2::QuerySetId::new(1),
                tables: vec![v2::TableUpdate {
                    table_name: "location_state".into(),
                    rows: vec![v2::TableUpdateRows::PersistentTable(
                        v2::PersistentTableRows {
                            deletes: rows(old),
                            inserts: rows(new),
                        },
                    )]
                    .into(),
                }]
                .into(),
            }]
            .into(),
        };
        c.replay_tx(DbUpdate::try_from(update).unwrap());
    };

    apply(None, Some(0)); // Never registered; row still enters the cache in cached mode.
    let obsolete = table.on_update(|_, _, _| panic!("removed update listener ran"));
    let keep = table.on_update(|_, a, b| SEEN.with_borrow_mut(|s| s.push((a.x, b.x))));
    table.remove_on_update(obsolete);
    c.replay_register();
    apply(Some(0), Some(1)); // One of two listeners removed.
    table.remove_on_update(keep);
    c.replay_register();
    apply(Some(1), Some(2)); // Last listener removed.
    let again = table.on_update(|_, a, b| SEEN.with_borrow_mut(|s| s.push((a.x, b.x))));
    c.replay_register();
    apply(Some(2), Some(3));
    table.remove_on_update(again);
    table.on_delete(|_, row| SEEN.with_borrow_mut(|s| s.push((row.x, -1))));
    c.replay_register();
    apply(Some(3), None);
    assert_eq!(SEEN.take(), [(0, 1), (2, 3), (3, -1)]);
}
