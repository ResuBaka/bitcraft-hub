// Public SDK client for the synthetic loopback test; no replay hooks.
use game_module::module_bindings::*;
use spacetimedb_sdk::{DbContext, Event, SubscriptionHandle as _, Table, TableWithPrimaryKey};
use std::cell::RefCell;

thread_local! { static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) }; }
fn record(event: &'static str) {
    EVENTS.with_borrow_mut(|events| events.push(event));
}

fn main() {
    let uri = std::env::args().nth(1).unwrap();
    for initial_ids in [vec![], vec![7]] {
        let has_initial = !initial_ids.is_empty();
        let db = DbConnection::builder()
            .with_uri(uri.as_str())
            .with_database_name("fixture")
            .on_connect(|_, _, _| record("connected"))
            .on_disconnect(|_, _| record("disconnected"))
            .build()
            .unwrap();
        let table = db.db.location_state();
        let ids = initial_ids.clone();
        table.on_initial_rows(move |ctx, rows| {
            assert!(matches!(ctx.event, Event::SubscribeApplied));
            assert_eq!(rows.iter().map(|r| r.entity_id).collect::<Vec<_>>(), ids);
            record("borrowed");
        });
        table.on_initial_owned(move |ctx, rows| {
            assert!(matches!(ctx.event, Event::SubscribeApplied));
            assert_eq!(
                rows.iter().map(|r| r.entity_id).collect::<Vec<_>>(),
                initial_ids
            );
            record("owned");
        });
        db.db.stamina_state().on_initial_rows(|ctx, rows| {
            assert!(matches!(ctx.event, Event::SubscribeApplied) && rows.is_empty());
            record("borrowed-empty");
        });
        db.db.stamina_state().on_initial_owned(|ctx, rows| {
            assert!(matches!(ctx.event, Event::SubscribeApplied) && rows.is_empty());
            record("owned-empty");
        });
        table.on_insert(|ctx, r| {
            assert!(matches!(ctx.event, Event::Transaction));
            assert_eq!((r.entity_id, r.x), (17, 1));
            record("insert");
        });
        table.on_update(|ctx, old, new| {
            assert!(matches!(ctx.event, Event::Reducer(_)));
            assert_eq!((old.x, new.x), (1, 2));
            record("update");
        });
        table.on_delete(|ctx, r| {
            assert!(matches!(ctx.event, Event::UnsubscribeApplied));
            match (r.entity_id, r.x) {
                (17, 2) => record("delete"),
                (7, 0) => record("delete-initial"),
                other => panic!("unexpected deletion: {other:?}"),
            }
        });
        let sub = db
            .subscription_builder()
            .on_applied(|_| record("applied"))
            .subscribe([
                "SELECT * FROM location_state",
                "SELECT * FROM stamina_state",
            ]);
        while !EVENTS.with_borrow(|e| e.ends_with(&["insert"])) {
            db.advance_one_message_blocking().unwrap();
        }
        db.reducers
            .ability_remove_then(0, 0, |_, result| {
                result.unwrap().unwrap();
                record("committed");
            })
            .unwrap();
        while !EVENTS.with_borrow(|e| e.ends_with(&["committed"])) {
            db.advance_one_message_blocking().unwrap();
        }
        sub.unsubscribe_then(Box::new(|_| record("ended"))).unwrap();
        while !EVENTS.with_borrow(|e| e.ends_with(&["disconnected"])) {
            if db.advance_one_message_blocking().is_err() {
                break;
            }
        }
        let events = EVENTS.take();
        let mut expected = vec![
            "connected",
            "applied",
            "borrowed",
            "borrowed-empty",
            "owned",
            "owned-empty",
            "insert",
            "update",
            "committed",
            "ended",
        ];
        if has_initial {
            expected.push("delete-initial");
        }
        expected.extend(["delete", "disconnected"]);
        assert_eq!(events, expected);
        println!("{}", events.join(","));
    }
}
