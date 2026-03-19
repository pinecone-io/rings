// Tests for schema migration (Task 2): FailureReason, AncestryInfo, CostEntry extensions

use rings::audit::CostEntry;
use rings::state::{AncestryInfo, FailureReason, RunMeta, StateFile};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn failure_reason_quota_deserialize() {
    let reason: FailureReason = serde_json::from_str("\"quota\"").unwrap();
    assert_eq!(reason, FailureReason::Quota);
}

#[test]
fn failure_reason_timeout_deserialize() {
    let reason: FailureReason = serde_json::from_str("\"timeout\"").unwrap();
    assert_eq!(reason, FailureReason::Timeout);
}

#[test]
fn failure_reason_auth_deserialize() {
    let reason: FailureReason = serde_json::from_str("\"auth\"").unwrap();
    assert_eq!(reason, FailureReason::Auth);
}

#[test]
fn failure_reason_unknown_deserialize() {
    let reason: FailureReason = serde_json::from_str("\"unknown\"").unwrap();
    assert_eq!(reason, FailureReason::Unknown);
}

#[test]
fn statefile_deserialize_with_failure_reason_quota() {
    let json = r#"{
        "schema_version": 1,
        "run_id": "run_test_123",
        "workflow_file": "/test/workflow.toml",
        "last_completed_run": 5,
        "last_completed_cycle": 1,
        "last_completed_phase_index": 0,
        "last_completed_iteration": 2,
        "total_runs_completed": 5,
        "cumulative_cost_usd": 0.05,
        "claude_resume_commands": [],
        "canceled_at": null,
        "failure_reason": "quota"
    }"#;

    let state: StateFile = serde_json::from_str(json).unwrap();
    assert_eq!(state.failure_reason, Some(FailureReason::Quota));
    assert_eq!(state.ancestry, None);
}

#[test]
fn statefile_deserialize_without_failure_reason() {
    let json = r#"{
        "schema_version": 1,
        "run_id": "run_test_123",
        "workflow_file": "/test/workflow.toml",
        "last_completed_run": 5,
        "last_completed_cycle": 1,
        "last_completed_phase_index": 0,
        "last_completed_iteration": 2,
        "total_runs_completed": 5,
        "cumulative_cost_usd": 0.05,
        "claude_resume_commands": [],
        "canceled_at": null
    }"#;

    let state: StateFile = serde_json::from_str(json).unwrap();
    assert_eq!(state.failure_reason, None);
    assert_eq!(state.ancestry, None);
}

#[test]
fn ancestry_info_in_statefile() {
    let json = r#"{
        "schema_version": 1,
        "run_id": "run_test_123",
        "workflow_file": "/test/workflow.toml",
        "last_completed_run": 5,
        "last_completed_cycle": 1,
        "last_completed_phase_index": 0,
        "last_completed_iteration": 2,
        "total_runs_completed": 5,
        "cumulative_cost_usd": 0.05,
        "claude_resume_commands": [],
        "canceled_at": null,
        "ancestry": {
            "parent_run_id": "run_parent_123",
            "continuation_of": "run_parent_123",
            "ancestry_depth": 1
        }
    }"#;

    let state: StateFile = serde_json::from_str(json).unwrap();
    assert!(state.ancestry.is_some());
    let ancestry = state.ancestry.unwrap();
    assert_eq!(ancestry.parent_run_id, Some("run_parent_123".to_string()));
    assert_eq!(ancestry.continuation_of, Some("run_parent_123".to_string()));
    assert_eq!(ancestry.ancestry_depth, 1);
}

#[test]
fn runmeta_deserialize_without_ancestry_fields() {
    let toml_str = r#"
run_id = "run_20240315_143022_a1b2c3"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T14:30:22Z"
rings_version = "0.1.0"
status = "running"
"#;

    let meta: RunMeta = toml::from_str(toml_str).unwrap();
    assert_eq!(meta.run_id, "run_20240315_143022_a1b2c3");
    assert_eq!(meta.parent_run_id, None);
    assert_eq!(meta.continuation_of, None);
    assert_eq!(meta.ancestry_depth, 0);
}

#[test]
fn runmeta_deserialize_with_ancestry_fields() {
    let toml_str = r#"
run_id = "run_20240315_150012_x9y8z7"
workflow_file = "/abs/path/to/my-task.rings.toml"
started_at = "2024-03-15T15:00:12Z"
rings_version = "0.1.0"
status = "running"
parent_run_id = "run_20240315_143022_a1b2c3"
continuation_of = "run_20240315_143022_a1b2c3"
ancestry_depth = 1
"#;

    let meta: RunMeta = toml::from_str(toml_str).unwrap();
    assert_eq!(
        meta.parent_run_id,
        Some("run_20240315_143022_a1b2c3".to_string())
    );
    assert_eq!(
        meta.continuation_of,
        Some("run_20240315_143022_a1b2c3".to_string())
    );
    assert_eq!(meta.ancestry_depth, 1);
}

#[test]
fn costentry_deserialize_without_file_diff_fields() {
    let json = r#"{
        "run": 1,
        "cycle": 1,
        "phase": "builder",
        "iteration": 1,
        "cost_usd": 0.0234,
        "input_tokens": 1234,
        "output_tokens": 567,
        "cost_confidence": "full"
    }"#;

    let entry: CostEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.run, 1);
    assert_eq!(entry.files_added, 0);
    assert_eq!(entry.files_modified, 0);
    assert_eq!(entry.files_deleted, 0);
    assert_eq!(entry.files_changed, 0);
    assert_eq!(entry.event, None);
}

#[test]
fn costentry_deserialize_with_file_diff_fields() {
    let json = r#"{
        "run": 1,
        "cycle": 1,
        "phase": "builder",
        "iteration": 1,
        "cost_usd": 0.0234,
        "input_tokens": 1234,
        "output_tokens": 567,
        "cost_confidence": "full",
        "files_added": 2,
        "files_modified": 3,
        "files_deleted": 1,
        "files_changed": 6,
        "event": "run_end"
    }"#;

    let entry: CostEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.files_added, 2);
    assert_eq!(entry.files_modified, 3);
    assert_eq!(entry.files_deleted, 1);
    assert_eq!(entry.files_changed, 6);
    assert_eq!(entry.event, Some("run_end".to_string()));
}

#[test]
fn costentry_file_diff_field_defaults() {
    let json = r#"{
        "run": 2,
        "cycle": 1,
        "phase": "reviewer",
        "iteration": 1,
        "cost_usd": 0.0198,
        "input_tokens": 1050,
        "output_tokens": 489,
        "cost_confidence": "full"
    }"#;

    let entry: CostEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.files_added, 0);
    assert_eq!(entry.files_modified, 0);
    assert_eq!(entry.files_deleted, 0);
    assert_eq!(entry.files_changed, 0);
    assert_eq!(entry.event, None);
}

#[test]
fn costentry_serialize_includes_all_fields() {
    let entry = CostEntry {
        run: 1,
        cycle: 1,
        phase: "builder".to_string(),
        iteration: 1,
        cost_usd: Some(0.05),
        input_tokens: Some(1000),
        output_tokens: Some(200),
        cost_confidence: "full".to_string(),
        files_added: 1,
        files_modified: 2,
        files_deleted: 0,
        files_changed: 3,
        event: Some("run_end".to_string()),
        produces_violations: vec![],
    };

    let json = serde_json::to_string(&entry).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["files_added"], 1);
    assert_eq!(parsed["files_modified"], 2);
    assert_eq!(parsed["files_deleted"], 0);
    assert_eq!(parsed["files_changed"], 3);
    assert_eq!(parsed["event"], "run_end");
}

#[test]
fn failure_reason_all_variants_serialize() {
    let quota = FailureReason::Quota;
    let auth = FailureReason::Auth;
    let timeout = FailureReason::Timeout;
    let unknown = FailureReason::Unknown;

    let quota_json = serde_json::to_string(&quota).unwrap();
    let auth_json = serde_json::to_string(&auth).unwrap();
    let timeout_json = serde_json::to_string(&timeout).unwrap();
    let unknown_json = serde_json::to_string(&unknown).unwrap();

    assert_eq!(quota_json, "\"quota\"");
    assert_eq!(auth_json, "\"auth\"");
    assert_eq!(timeout_json, "\"timeout\"");
    assert_eq!(unknown_json, "\"unknown\"");
}

#[test]
fn failure_reason_all_variants_deserialize() {
    let quota: FailureReason = serde_json::from_str("\"quota\"").unwrap();
    let auth: FailureReason = serde_json::from_str("\"auth\"").unwrap();
    let timeout: FailureReason = serde_json::from_str("\"timeout\"").unwrap();
    let unknown: FailureReason = serde_json::from_str("\"unknown\"").unwrap();

    assert_eq!(quota, FailureReason::Quota);
    assert_eq!(auth, FailureReason::Auth);
    assert_eq!(timeout, FailureReason::Timeout);
    assert_eq!(unknown, FailureReason::Unknown);
}

#[test]
fn statefile_full_roundtrip_with_ancestry() {
    let original = StateFile {
        schema_version: 1,
        run_id: "run_test_123".to_string(),
        workflow_file: "/test/workflow.toml".to_string(),
        last_completed_run: 5,
        last_completed_cycle: 2,
        last_completed_phase_index: 1,
        last_completed_iteration: 2,
        total_runs_completed: 5,
        cumulative_cost_usd: 0.15,
        claude_resume_commands: vec![],
        canceled_at: None,
        failure_reason: Some(FailureReason::Quota),
        ancestry: Some(AncestryInfo {
            parent_run_id: Some("run_parent_123".to_string()),
            continuation_of: Some("run_parent_123".to_string()),
            ancestry_depth: 1,
        }),
    };

    let json = serde_json::to_string(&original).unwrap();
    let deserialized: StateFile = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.run_id, original.run_id);
    assert_eq!(deserialized.failure_reason, original.failure_reason);
    assert!(deserialized.ancestry.is_some());
    let ancestry = deserialized.ancestry.unwrap();
    assert_eq!(ancestry.parent_run_id, Some("run_parent_123".to_string()));
    assert_eq!(ancestry.ancestry_depth, 1);
}

#[test]
fn costentry_stream_backwards_compat() {
    // Test that we can read old cost entries without the new fields
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"{{"run":1,"cycle":1,"phase":"builder","iteration":1,"cost_usd":0.05,"input_tokens":1000,"output_tokens":200,"cost_confidence":"full"}}"#
    )
    .unwrap();
    writeln!(
        temp_file,
        r#"{{"run":2,"cycle":1,"phase":"builder","iteration":2,"cost_usd":0.03,"input_tokens":800,"output_tokens":150,"cost_confidence":"full"}}"#
    )
    .unwrap();

    let path = temp_file.path();
    let entries: Vec<_> = rings::audit::stream_cost_entries(path)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].run, 1);
    assert_eq!(entries[0].files_added, 0); // defaults to 0
    assert_eq!(entries[0].event, None); // defaults to None
    assert_eq!(entries[1].run, 2);
}
