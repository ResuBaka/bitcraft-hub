use super::*;

#[tokio::test]
async fn category_rescan_uses_current_definitions_for_unchanged_stacks() {
    let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::new(Default::default(), tx, Default::default());
    let _subscriber = subscribe_xp(&state).await;
    let define = |id: i32, name: &str, category| {
        state.skill_desc.write().insert(
            i64::from(id),
            entity::skill_desc::Model {
                id: i64::from(id),
                skill: id,
                name: name.into(),
                skill_category: category,
                description: String::new(),
                icon_asset_name: String::new(),
                title: String::new(),
            },
        );
    };
    define(0, "first", 1);
    define(63, "last", 2);
    define(64, "sparse", 1);
    define(-1, "negative", 1);
    define(9, "other", 257); // must not narrow to profession
    let mut stacks = [
        (0, 10),
        (63, 20),
        (64, 30),
        (-1, 40),
        (9, 50),
        (i32::MAX, 60),
        (i32::MIN, 70),
        (12, 80),
    ];
    let mut worker = ExperienceStateWorker::new(state.clone(), 100, Duration::from_secs(10));
    worker.handle_insert(experience(1, &stacks), 7).await;
    let rank = &state.ranking_system;
    assert_eq!(rank.profession_leaderboard.get_value(&1), Some(80));
    assert_eq!(rank.adventure_leaderboard.get_value(&1), Some(20));

    define(63, "promoted", 1);
    define(i32::MIN, "far negative", 1);
    state.skill_desc.write().remove(&64);
    rank.profession_leaderboard.update(1, 9999);
    let old = experience(1, &stacks);
    stacks[3].1 += 1;
    worker.handle_update(experience(1, &stacks), 7, old).await;
    assert_eq!(rank.skill_leaderboard(-1).unwrap().get_value(&1), Some(41));
    assert_eq!(rank.profession_leaderboard.get_value(&1), Some(141));
    assert_eq!(rank.adventure_leaderboard.get_value(&1), Some(20));
    assert!(
        matches!(events.try_recv().unwrap(), OutboundWebSocketMessages::Experience {
        skill_name, experience: 41, .. } if skill_name == "negative")
    );
    assert!(matches!(
        events.try_recv().unwrap(),
        OutboundWebSocketMessages::TotalExperience {
            experience: 361,
            ..
        }
    ));
    assert!(matches!(
        events.try_recv().unwrap(),
        OutboundWebSocketMessages::TotalProfessionExperience {
            experience: 141,
            ..
        }
    ));
    assert!(events.try_recv().is_err());

    define(0, "renamed", 2);
    define(i32::MAX, "far positive", 2);
    let old = experience(1, &stacks);
    stacks[0].1 += 1;
    worker.handle_update(experience(1, &stacks), 7, old).await;
    assert_eq!(rank.adventure_leaderboard.get_value(&1), Some(71));
    assert_eq!(rank.profession_leaderboard.get_value(&1), Some(141));
    assert!(
        matches!(events.try_recv().unwrap(), OutboundWebSocketMessages::Experience {
        skill_name, experience: 11, .. } if skill_name == "renamed")
    );
    assert!(matches!(
        events.try_recv().unwrap(),
        OutboundWebSocketMessages::TotalExperience {
            experience: 362,
            ..
        }
    ));
    assert!(matches!(
        events.try_recv().unwrap(),
        OutboundWebSocketMessages::TotalAdventureExperience { experience: 71, .. }
    ));
    assert!(events.try_recv().is_err());

    state.skill_desc.write().remove(&0);
    let old = experience(1, &stacks);
    stacks[0].1 += 1;
    worker.handle_update(experience(1, &stacks), 7, old).await;
    assert_eq!(rank.skill_leaderboard(0).unwrap().get_value(&1), Some(11));
    assert_eq!(rank.adventure_leaderboard.get_value(&1), Some(71));
    assert_eq!(worker.pending.upserts[&(1, 0)].experience, 12);
    assert_eq!(
        worker
            .pending
            .claim_history
            .iter()
            .map(|e| (e.skill_id, e.previous_experience, e.experience))
            .collect::<Vec<_>>(),
        [(-1, 40, 41), (0, 10, 11), (0, 11, 12)]
    );
    assert!(matches!(
        events.try_recv().unwrap(),
        OutboundWebSocketMessages::TotalExperience {
            experience: 363,
            ..
        }
    ));
    assert!(events.try_recv().is_err());
}
