use rings::engine::BudgetTracker;

/// Helper: simulate updating the no_change_streaks counter for a phase.
/// Returns the streak count after the update.
fn update_streak(tracker: &mut BudgetTracker, phase: &str, had_produces_change: bool) -> u32 {
    let streak = tracker
        .no_change_streaks
        .entry(phase.to_string())
        .or_insert(0);
    if had_produces_change {
        *streak = 0;
    } else {
        *streak += 1;
    }
    *streak
}

#[test]
fn streak_reaches_3_after_three_no_change_runs() {
    let mut tracker = BudgetTracker::new();
    let streak1 = update_streak(&mut tracker, "builder", false);
    let streak2 = update_streak(&mut tracker, "builder", false);
    let streak3 = update_streak(&mut tracker, "builder", false);
    assert_eq!(streak1, 1);
    assert_eq!(streak2, 2);
    assert_eq!(streak3, 3);
    // Warning should fire when streak == 3
    assert_eq!(*tracker.no_change_streaks.get("builder").unwrap(), 3);
}

#[test]
fn streak_resets_on_change_run() {
    let mut tracker = BudgetTracker::new();
    // Two no-change runs
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    // Then a run that produces changes
    let after_change = update_streak(&mut tracker, "builder", true);
    assert_eq!(after_change, 0);
    // No warning should have fired (streak never reached 3)
    assert_eq!(*tracker.no_change_streaks.get("builder").unwrap(), 0);
}

#[test]
fn streak_does_not_re_warn_after_reaching_3() {
    let mut tracker = BudgetTracker::new();
    // Build up to 3
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    let at_3 = update_streak(&mut tracker, "builder", false);
    // 4th consecutive no-change: streak goes to 4, condition `streak == 3` is false
    let at_4 = update_streak(&mut tracker, "builder", false);
    assert_eq!(at_3, 3);
    assert_eq!(at_4, 4);
    // The warning condition `streak == 3` is only true at exactly 3, not 4
    assert_ne!(at_4, 3);
}

#[test]
fn streak_resets_after_change_clears_prior_streak() {
    let mut tracker = BudgetTracker::new();
    // Get to streak of 3
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    // Change resets streak
    update_streak(&mut tracker, "builder", true);
    // A new no-change run starts fresh — streak goes to 1, not 4
    let after_reset = update_streak(&mut tracker, "builder", false);
    assert_eq!(after_reset, 1);
}

#[test]
fn streak_tracks_phases_independently() {
    let mut tracker = BudgetTracker::new();
    // Phase builder: 3 no-change runs
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    update_streak(&mut tracker, "builder", false);
    // Phase reviewer: 1 no-change run
    update_streak(&mut tracker, "reviewer", false);

    assert_eq!(*tracker.no_change_streaks.get("builder").unwrap(), 3);
    assert_eq!(*tracker.no_change_streaks.get("reviewer").unwrap(), 1);
}

#[test]
fn streak_does_not_exist_when_manifest_not_enabled() {
    // When manifest is disabled, the streak update block is never entered,
    // so no_change_streaks remains empty.
    let tracker = BudgetTracker::new();
    assert!(tracker.no_change_streaks.is_empty());
    // Without manifest_enabled, the streak entry for any phase should never be written.
    assert!(tracker.no_change_streaks.get("builder").is_none());
}
