//! A username belongs to the player, but its read model also exposes the
//! player's current region. A transfer changes user_region_state without
//! necessarily changing the name. Resolve the join after all row callbacks
//! for an SDK message, independent of table order.
use super::{
    SpacetimeUpdateMessages as Message, initial_batches, send_worker_message,
    worker_channel::UnboundedSender,
};
use entity::shared::Region;
use game_global_module::module_bindings::*;
use game_module::module_bindings::PlayerUsernameState as Name;
use parking_lot::Mutex;
use spacetimedb_sdk::{Event, Identity, Table, TableWithPrimaryKey};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Default)]
pub(super) struct Directory {
    ids: HashMap<Identity, u64>,
    identities: HashMap<u64, Identity>,
    regions: HashMap<Identity, Region>,
    names: HashMap<u64, String>,
    published: HashMap<u64, Region>,
    changed: HashSet<u64>,
}

impl Directory {
    fn user(&mut self, row: &UserState, present: bool) {
        if present {
            self.ids.insert(row.identity, row.entity_id);
            self.identities.insert(row.entity_id, row.identity);
        } else {
            self.ids.remove(&row.identity);
            self.identities.remove(&row.entity_id);
        }
        self.changed.insert(row.entity_id);
    }

    fn region(&mut self, row: &UserRegionState, present: bool) {
        if present {
            self.regions.insert(row.identity, row.region_id.into());
        } else {
            self.regions.remove(&row.identity);
        }
        if let Some(id) = self.ids.get(&row.identity) {
            self.changed.insert(*id);
        }
    }

    fn name(&mut self, row: &PlayerUsernameState, present: bool) {
        if present {
            self.names.insert(row.entity_id, row.username.clone());
        } else {
            self.names.remove(&row.entity_id);
        }
        self.changed.insert(row.entity_id);
    }

    pub(super) fn publish(
        &mut self,
        tx: &UnboundedSender<Message<Name>>,
        regions: &[Region],
        initial: bool,
    ) {
        let mut batches = HashMap::new();
        for id in self.changed.drain() {
            let region = self
                .identities
                .get(&id)
                .and_then(|identity| self.regions.get(identity));
            if let Some((&region, name)) = region.zip(self.names.get(&id)) {
                let name = Name {
                    entity_id: id,
                    username: name.clone(),
                };
                self.published.insert(id, region);
                if initial {
                    batches.entry(region).or_insert_with(Vec::new).push(name);
                } else {
                    send_worker_message(
                        "player_username_state",
                        tx,
                        Message::Insert {
                            database_name: region,
                            new: name,
                            event: None,
                        },
                    );
                }
            } else if let Some(region) = self.published.remove(&id) {
                send_worker_message(
                    "player_username_state",
                    tx,
                    Message::Remove {
                        database_name: region,
                        delete: Name {
                            entity_id: id,
                            username: String::new(),
                        },
                        event: None,
                    },
                );
            }
        }
        if initial {
            for &region in regions {
                batches.entry(region).or_default();
            }
            for (region, rows) in batches {
                initial_batches::send_initial("player_username_state", tx, rows, region, 0);
            }
        }
    }

    pub(super) fn listen(ctx: &DbConnection) -> Arc<Mutex<Self>> {
        let rows = Arc::new(Mutex::new(Self::default()));
        macro_rules! track {
            ($table:ident, $change:ident, $same:expr) => {{
                let table = ctx.db.$table();
                let target = rows.clone();
                table.on_initial(move |_, batch| {
                    let mut target = target.lock();
                    for row in batch {
                        target.$change(row, true);
                    }
                });
                let target = rows.clone();
                table.on_insert(move |ctx, row| {
                    if !matches!(ctx.event, Event::SubscribeApplied) {
                        target.lock().$change(row, true);
                    }
                });
                let target = rows.clone();
                table.on_update(move |_, old, new| {
                    if ($same)(old, new) {
                        return;
                    }
                    let mut target = target.lock();
                    target.$change(old, false);
                    target.$change(new, true);
                });
                let target = rows.clone();
                table.on_delete(move |_, row| target.lock().$change(row, false));
            }};
        }
        track!(user_state, user, |a: &UserState, b: &UserState| a.identity
            == b.identity
            && a.entity_id == b.entity_id);
        track!(user_region_state, region, PartialEq::eq);
        track!(player_username_state, name, PartialEq::eq);
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name_region_join_handles_unordered_snapshots_transfers_and_removals() {
        let (tx, mut rx) = super::super::worker_channel::unbounded_channel();
        let mut directory = Directory::default();
        let identity = Identity::from_byte_array([1; 32]);
        let name = PlayerUsernameState {
            entity_id: 123,
            username: "Ada".into(),
        };
        let mut location = UserRegionState {
            identity,
            region_id: 7,
        };
        directory.name(&name, true);
        directory.region(&location, true);
        directory.user(
            &UserState {
                identity,
                entity_id: 123,
                can_sign_in: true,
            },
            true,
        );
        directory.publish(&tx, &[7, 8], true);
        let mut batches = HashMap::new();
        while let Ok(Message::Initial {
            database_name,
            data,
            ..
        }) = rx.try_recv()
        {
            batches.insert(database_name, data);
        }
        assert_eq!(batches[&7][0].username, "Ada");
        assert!(batches[&8].is_empty());
        location.region_id = 8;
        directory.region(&location, true);
        directory.publish(&tx, &[], false);
        assert!(
            matches!(rx.try_recv(), Ok(Message::Insert {database_name: 8, new, ..}) if new.entity_id == 123)
        );
        directory.name(&name, false);
        directory.publish(&tx, &[], false);
        assert!(
            matches!(rx.try_recv(), Ok(Message::Remove {database_name: 8, delete, ..}) if delete.entity_id == 123)
        );
        assert!(rx.is_empty());
    }
}
