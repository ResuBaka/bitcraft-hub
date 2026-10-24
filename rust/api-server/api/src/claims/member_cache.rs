//! Only the outer maps need sharding. Their guards protect each compact inner
//! collection; never hold a guard while acquiring the other index.
use dashmap::{DashMap, mapref::entry::Entry};
use entity::claim_member_state::Model;
use std::collections::{HashMap, HashSet};

pub(crate) type MembersByClaim = DashMap<u64, HashMap<i64, Model>>;
pub(crate) type ClaimsByPlayer = DashMap<u64, HashSet<u64>>;

pub(crate) fn insert(members: &MembersByClaim, players: &ClaimsByPlayer, model: Model) {
    let claim_id = model.claim_entity_id as u64;
    let player_id = model.player_entity_id as u64;
    members
        .entry(claim_id)
        .or_default()
        .insert(model.entity_id, model);
    players.entry(player_id).or_default().insert(claim_id);
}

pub(crate) fn remove(members: &MembersByClaim, players: &ClaimsByPlayer, model: &Model) {
    let claim_id = model.claim_entity_id as u64;
    let player_id = model.player_entity_id as u64;
    if let Entry::Occupied(mut entry) = members.entry(claim_id) {
        entry.get_mut().remove(&model.entity_id);
        if entry
            .get()
            .values()
            .any(|member| member.player_entity_id == model.player_entity_id)
        {
            return;
        }
        if entry.get().is_empty() {
            entry.remove();
        }
    }
    if let Entry::Occupied(mut entry) = players.entry(player_id) {
        entry.get_mut().remove(&claim_id);
        if entry.get().is_empty() {
            entry.remove();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: i64, claim: i64, player: i64) -> Model {
        Model {
            entity_id: id,
            claim_entity_id: claim,
            player_entity_id: player,
            user_name: format!("player-{player}"),
            inventory_permission: true,
            build_permission: false,
            officer_permission: false,
            co_owner_permission: false,
            region: 1,
        }
    }

    #[test]
    fn insert_replaces_members_and_deduplicates_reverse_index() {
        let members = MembersByClaim::new();
        let players = ClaimsByPlayer::new();
        let mut row = model(1, 10, 100);
        insert(&members, &players, row.clone());
        row.user_name = "updated".into();
        row.build_permission = true;
        insert(&members, &players, row.clone());
        assert_eq!(members.get(&10).unwrap().len(), 1);
        assert_eq!(members.get(&10).unwrap().get(&1), Some(&row));
        assert_eq!(*players.get(&100).unwrap(), HashSet::from([10]));
    }

    #[test]
    fn removal_uses_player_id_preserves_other_memberships_and_prunes_empty_entries() {
        let members = MembersByClaim::new();
        let players = ClaimsByPlayer::new();
        let first = model(1, 10, 100);
        let other_claim = model(2, 20, 100);
        let other_player = model(3, 10, 200);
        for row in [&first, &other_claim, &other_player] {
            insert(&members, &players, row.clone());
        }
        remove(&members, &players, &first);
        assert_eq!(*players.get(&100).unwrap(), HashSet::from([20]));
        assert_eq!(members.get(&10).unwrap().len(), 1);
        assert!(members.get(&10).unwrap().contains_key(&3));
        remove(&members, &players, &other_claim);
        assert!(!members.contains_key(&20));
        assert!(!players.contains_key(&100));
        remove(&members, &players, &other_player);
        remove(&members, &players, &other_player); // idempotent
        assert!(members.is_empty());
        assert!(players.is_empty());
    }

    #[test]
    fn reverse_entry_survives_another_membership_for_the_same_player_and_claim() {
        let members = MembersByClaim::new();
        let players = ClaimsByPlayer::new();
        let first = model(1, 10, 100);
        let second = model(2, 10, 100);
        insert(&members, &players, first.clone());
        insert(&members, &players, second.clone());
        remove(&members, &players, &first);
        assert!(players.get(&100).unwrap().contains(&10));
        remove(&members, &players, &second);
        assert!(players.is_empty());
        assert!(members.is_empty());
    }

    #[test]
    fn concurrent_first_inserts_do_not_overwrite_each_other() {
        let members = MembersByClaim::new();
        let players = ClaimsByPlayer::new();
        std::thread::scope(|scope| {
            for worker in 0..8 {
                let members = &members;
                let players = &players;
                scope.spawn(move || {
                    for index in 0..100 {
                        let id = worker * 100 + index;
                        insert(members, players, model(id, 10, 100));
                        insert(members, players, model(id + 1000, id + 20, 100));
                    }
                });
            }
        });
        assert_eq!(members.get(&10).unwrap().len(), 800);
        assert_eq!(players.get(&100).unwrap().len(), 801);
    }
}
