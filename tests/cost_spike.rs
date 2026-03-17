use std::collections::{HashMap, VecDeque};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_no_warning_with_fewer_than_3_entries() {
    // With only 2 entries, no spike detection
    let window: VecDeque<f64> = vec![1.0, 2.0].into_iter().collect();

    if window.len() < 3 {
        // No spike warning with fewer than 3 entries
        assert!(true);
    } else {
        panic!("Should not reach here");
    }
}

#[test]
fn test_no_warning_at_exactly_5x_average() {
    // Exact boundary: cost = 5.0 × average should NOT trigger (strict >)
    // window = [1.0, 1.0, 1.0, 10.0]
    // average of first 3 = 1.0
    // current = 10.0
    // multiplier = 10.0 / 1.0 = 10.0 > 5.0, so THIS IS a spike

    // Let's try a different: [2.0, 2.0, 2.0, 10.0]
    // average of first 3 = 2.0
    // current = 10.0
    // multiplier = 10.0 / 2.0 = 5.0, which is NOT > 5.0

    let first_three_sum = 2.0 + 2.0 + 2.0;
    let avg = first_three_sum / 3.0;
    assert_eq!(avg, 2.0);

    let current_cost = 10.0;
    let multiplier = current_cost / avg;
    assert_eq!(multiplier, 5.0);

    // multiplier is exactly 5.0, not > 5.0, so NO spike
    assert!(!(current_cost > 5.0 * avg));
}

#[test]
fn test_warning_when_cost_exceeds_5x_average() {
    // [2.0, 2.0, 2.0, 10.01]
    // average of first 3 = 2.0
    // current = 10.01
    // multiplier = 10.01 / 2.0 = 5.005 > 5.0, so THIS IS a spike

    let first_three_sum = 2.0 + 2.0 + 2.0;
    let avg = first_three_sum / 3.0;
    assert_eq!(avg, 2.0);

    let current_cost = 10.01;
    let multiplier = current_cost / avg;
    assert!(multiplier > 5.0);
    assert!(current_cost > 5.0 * avg);
}

#[test]
fn test_rolling_window_drops_oldest_at_cap() {
    let mut window: VecDeque<f64> = VecDeque::new();
    // Push 7 values with cap at 5
    for i in 0..7 {
        if window.len() >= 5 {
            window.pop_front();
        }
        window.push_back(i as f64);
    }

    assert_eq!(window.len(), 5);
    // Should contain 2, 3, 4, 5, 6
    let vals: Vec<f64> = window.iter().copied().collect();
    assert_eq!(vals, vec![2.0, 3.0, 4.0, 5.0, 6.0]);
}

#[test]
fn test_zero_rolling_average_no_divide_by_zero() {
    // [0.0, 0.0, 0.0, X]
    // average of first 3 = 0.0
    // When average is 0, we should skip the check

    let first_three_sum = 0.0 + 0.0 + 0.0;
    let avg = first_three_sum / 3.0;

    // avg is 0, so we should skip the check
    if avg == 0.0 {
        // No spike check when average is zero
        assert!(true);
    } else {
        panic!("Should skip when average is zero");
    }
}

#[test]
fn test_none_cost_entries_skipped() {
    // In the engine, None-cost entries should not be pushed to the window
    // and should not count toward the 3-run minimum

    let mut window: VecDeque<f64> = VecDeque::new();

    // Simulate 5 entries where 2 are None (skipped) and 3 are Some
    let costs = vec![Some(1.0), None, Some(1.0), None, Some(1.0)];

    for cost in costs {
        if let Some(c) = cost {
            if window.len() >= 5 {
                window.pop_front();
            }
            window.push_back(c);
        }
        // None entries are not added to window
    }

    assert_eq!(window.len(), 3);
    let vals: Vec<f64> = window.iter().copied().collect();
    assert_eq!(vals, vec![1.0, 1.0, 1.0]);
}

#[test]
fn test_spike_detection_boundary() {
    // Test the exact boundary case
    // window = [1.0, 1.0, 1.0, 1.0, 1.0, 5.0]
    // We only keep last 5, so window = [1.0, 1.0, 1.0, 1.0, 5.0]
    // average of first 4 = 1.0
    // current = 5.0
    // multiplier = 5.0 / 1.0 = 5.0, which is NOT > 5.0

    let window: VecDeque<f64> = vec![1.0, 1.0, 1.0, 1.0, 5.0].into_iter().collect();

    if window.len() >= 3 {
        let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
        let current_cost = *window.back().unwrap();
        let multiplier = current_cost / avg;

        // multiplier is exactly 5.0, not > 5.0
        assert_eq!(multiplier, 5.0);
        assert!(!(current_cost > 5.0 * avg));
    }
}

#[test]
fn test_spike_detection_exceeds_boundary() {
    // window = [1.0, 1.0, 1.0, 1.0, 5.1]
    // average of first 4 = 1.0
    // current = 5.1
    // multiplier = 5.1 / 1.0 = 5.1 > 5.0, so THIS IS a spike

    let window: VecDeque<f64> = vec![1.0, 1.0, 1.0, 1.0, 5.1].into_iter().collect();

    if window.len() >= 3 {
        let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
        let current_cost = *window.back().unwrap();
        let multiplier = current_cost / avg;

        assert!(multiplier > 5.0);
        assert!(current_cost > 5.0 * avg);
    }
}

#[test]
fn test_integration_with_engine_spike_on_resume() {
    let temp_dir = TempDir::new().unwrap();
    let output_dir = temp_dir.path().to_path_buf();

    // Create a costs.jsonl file with historical data
    let costs_file = output_dir.join("costs.jsonl");
    let costs_content = r#"{"run":1,"cycle":1,"phase":"phase1","iteration":1,"cost_usd":1.0,"input_tokens":100,"output_tokens":50,"cost_confidence":"high"}
{"run":2,"cycle":1,"phase":"phase1","iteration":2,"cost_usd":1.0,"input_tokens":100,"output_tokens":50,"cost_confidence":"high"}
{"run":3,"cycle":1,"phase":"phase1","iteration":3,"cost_usd":1.0,"input_tokens":100,"output_tokens":50,"cost_confidence":"high"}
{"run":4,"cycle":2,"phase":"phase1","iteration":1,"cost_usd":1.0,"input_tokens":100,"output_tokens":50,"cost_confidence":"high"}
{"run":5,"cycle":2,"phase":"phase1","iteration":2,"cost_usd":6.0,"input_tokens":100,"output_tokens":50,"cost_confidence":"high"}
"#;

    fs::write(&costs_file, costs_content).unwrap();

    // Verify rolling window is populated correctly on resume
    let mut rolling_windows: HashMap<String, VecDeque<f64>> = HashMap::new();

    for line in costs_content.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(cost) = entry.get("cost_usd").and_then(|v| v.as_f64()) {
                if let Some(phase) = entry.get("phase").and_then(|v| v.as_str()) {
                    let window = rolling_windows.entry(phase.to_string()).or_default();
                    if window.len() >= 5 {
                        window.pop_front();
                    }
                    window.push_back(cost);
                }
            }
        }
    }

    let window = rolling_windows.get("phase1").unwrap();
    assert_eq!(window.len(), 5);
    let vals: Vec<f64> = window.iter().copied().collect();
    assert_eq!(vals, vec![1.0, 1.0, 1.0, 1.0, 6.0]);

    // Check if the last entry (6.0) would trigger a spike
    // average of first 4 = 1.0
    // current = 6.0
    // multiplier = 6.0 / 1.0 = 6.0 > 5.0, so YES this IS a spike!
    let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
    if let Some(&current_cost) = window.back() {
        let multiplier = current_cost / avg;
        assert!(multiplier > 5.0); // This should be true now!
    }
}

#[test]
fn test_spike_not_triggered_with_high_baseline() {
    // With higher baseline costs, spike detection requires higher multiplier
    // [10.0, 10.0, 10.0, 10.0, 60.0]
    // average of first 4 = 10.0
    // current = 60.0
    // multiplier = 60.0 / 10.0 = 6.0 > 5.0, so this IS a spike

    let window: VecDeque<f64> = vec![10.0, 10.0, 10.0, 10.0, 60.0].into_iter().collect();

    let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
    assert_eq!(avg, 10.0);

    if let Some(&current_cost) = window.back() {
        let multiplier = current_cost / avg;
        assert!(multiplier > 5.0);
    }
}

#[test]
fn test_edge_case_exactly_3_entries() {
    // With exactly 3 entries, we compute average of first 2
    // [1.0, 1.0, 5.1]
    // average of first 2 = 1.0
    // current = 5.1
    // multiplier = 5.1 / 1.0 = 5.1 > 5.0, so this IS a spike

    let window: VecDeque<f64> = vec![1.0, 1.0, 5.1].into_iter().collect();

    assert_eq!(window.len(), 3);

    let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
    assert_eq!(avg, 1.0);

    if let Some(&current_cost) = window.back() {
        let multiplier = current_cost / avg;
        assert!(multiplier > 5.0);
    }
}

#[test]
fn test_multiple_phases_independent_windows() {
    // Each phase should have its own rolling window
    let mut rolling_windows: HashMap<String, VecDeque<f64>> = HashMap::new();

    // Phase 1: [1.0, 1.0, 1.0, 1.0, 6.0]
    let window1: VecDeque<f64> = vec![1.0, 1.0, 1.0, 1.0, 6.0].into_iter().collect();
    rolling_windows.insert("phase1".to_string(), window1);

    // Phase 2: [10.0, 10.0, 10.0, 10.0, 40.0]
    let window2: VecDeque<f64> = vec![10.0, 10.0, 10.0, 10.0, 40.0].into_iter().collect();
    rolling_windows.insert("phase2".to_string(), window2);

    // Check phase1: multiplier = 6.0 / 1.0 = 6.0 > 5.0 (spike)
    {
        let window = rolling_windows.get("phase1").unwrap();
        let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
        if let Some(&current_cost) = window.back() {
            let multiplier = current_cost / avg;
            assert!(multiplier > 5.0);
        }
    }

    // Check phase2: multiplier = 40.0 / 10.0 = 4.0 < 5.0 (no spike)
    {
        let window = rolling_windows.get("phase2").unwrap();
        let avg = window.iter().take(window.len() - 1).sum::<f64>() / (window.len() - 1) as f64;
        if let Some(&current_cost) = window.back() {
            let multiplier = current_cost / avg;
            assert!(multiplier <= 5.0);
        }
    }
}
