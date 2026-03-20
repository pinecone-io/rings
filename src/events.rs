use chrono::Utc;
use serde::Serialize;
use std::cell::RefCell;

/// Returns the current time as an ISO 8601 string (UTC).
pub fn now_iso8601() -> String {
    Utc::now().to_rfc3339()
}

thread_local! {
    /// When `Some`, `emit_jsonl` captures events here instead of printing to stdout.
    static CAPTURED_EVENTS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Start capturing JSONL events emitted on this thread (for tests only).
#[cfg(any(test, feature = "testing"))]
pub fn start_capture() {
    CAPTURED_EVENTS.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Stop capturing and return the captured event lines (for tests only).
#[cfg(any(test, feature = "testing"))]
pub fn stop_capture() -> Vec<String> {
    CAPTURED_EVENTS.with(|c| c.borrow_mut().take().unwrap_or_default())
}

/// Serializes an event to a single-line JSON string and prints it to stdout.
/// In test mode, if capture is active, events are stored instead of printed.
pub fn emit_jsonl(event: &impl Serialize) {
    if let Ok(s) = serde_json::to_string(event) {
        let mut captured = false;
        CAPTURED_EVENTS.with(|c| {
            if let Some(ref mut events) = *c.borrow_mut() {
                events.push(s.clone());
                captured = true;
            }
        });
        if !captured {
            println!("{}", s);
        }
    }
}

/// `start` — emitted once at the beginning of a workflow run.
#[derive(Debug, Serialize)]
pub struct StartEvent {
    pub event: &'static str,
    pub run_id: String,
    pub workflow: String,
    pub rings_version: &'static str,
    pub schema_version: u32,
    pub timestamp: String,
}

impl StartEvent {
    pub fn new(run_id: impl Into<String>, workflow: impl Into<String>) -> Self {
        Self {
            event: "start",
            run_id: run_id.into(),
            workflow: workflow.into(),
            rings_version: env!("CARGO_PKG_VERSION"),
            schema_version: 1,
            timestamp: now_iso8601(),
        }
    }
}

/// `run_start` — emitted before each executor spawn.
#[derive(Debug, Serialize)]
pub struct RunStartEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub cycle: u64,
    pub phase: String,
    pub iteration: u64,
    pub total_iterations: u64,
    pub template_context: serde_json::Value,
    pub timestamp: String,
}

impl RunStartEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        run: u64,
        cycle: u64,
        phase: impl Into<String>,
        iteration: u64,
        total_iterations: u64,
        template_context: serde_json::Value,
    ) -> Self {
        Self {
            event: "run_start",
            run_id: run_id.into(),
            run,
            cycle,
            phase: phase.into(),
            iteration,
            total_iterations,
            template_context,
            timestamp: now_iso8601(),
        }
    }
}

/// `run_end` — emitted after each executor run completes.
#[derive(Debug, Serialize)]
pub struct RunEndEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub cycle: u64,
    pub phase: String,
    pub iteration: u64,
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub exit_code: i32,
    pub produces_violations: Vec<String>,
    pub cost_confidence: String,
    pub total_iterations: u64,
    pub files_added: Vec<String>,
    pub files_modified: Vec<String>,
    pub files_deleted: Vec<String>,
    pub files_changed: u32,
    pub timestamp: String,
}

impl RunEndEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        run: u64,
        cycle: u64,
        phase: impl Into<String>,
        iteration: u64,
        cost_usd: Option<f64>,
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        exit_code: i32,
        produces_violations: Vec<String>,
        cost_confidence: impl Into<String>,
        total_iterations: u64,
        files_added: Vec<String>,
        files_modified: Vec<String>,
        files_deleted: Vec<String>,
        files_changed: u32,
    ) -> Self {
        Self {
            event: "run_end",
            run_id: run_id.into(),
            run,
            cycle,
            phase: phase.into(),
            iteration,
            cost_usd,
            input_tokens,
            output_tokens,
            exit_code,
            produces_violations,
            cost_confidence: cost_confidence.into(),
            total_iterations,
            files_added,
            files_modified,
            files_deleted,
            files_changed,
            timestamp: now_iso8601(),
        }
    }
}

/// `completion_signal` — emitted when the completion signal is detected in output.
#[derive(Debug, Serialize)]
pub struct CompletionSignalEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub cycle: u64,
    pub phase: String,
    pub signal: String,
    pub timestamp: String,
}

impl CompletionSignalEvent {
    pub fn new(
        run_id: impl Into<String>,
        run: u64,
        cycle: u64,
        phase: impl Into<String>,
        signal: impl Into<String>,
    ) -> Self {
        Self {
            event: "completion_signal",
            run_id: run_id.into(),
            run,
            cycle,
            phase: phase.into(),
            signal: signal.into(),
            timestamp: now_iso8601(),
        }
    }
}

/// `executor_error` — emitted when the executor subprocess exits non-zero.
#[derive(Debug, Serialize)]
pub struct ExecutorErrorEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub cycle: u64,
    pub phase: String,
    pub error_class: String,
    pub exit_code: i32,
    pub message: String,
    pub timestamp: String,
}

impl ExecutorErrorEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        run: u64,
        cycle: u64,
        phase: impl Into<String>,
        error_class: impl Into<String>,
        exit_code: i32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            event: "executor_error",
            run_id: run_id.into(),
            run,
            cycle,
            phase: phase.into(),
            error_class: error_class.into(),
            exit_code,
            message: message.into(),
            timestamp: now_iso8601(),
        }
    }
}

/// `canceled` — emitted when the user cancels execution (Ctrl+C).
#[derive(Debug, Serialize)]
pub struct CanceledEvent {
    pub event: &'static str,
    pub run_id: String,
    pub runs_completed: u64,
    pub cost_usd: f64,
    pub timestamp: String,
}

impl CanceledEvent {
    pub fn new(run_id: impl Into<String>, runs_completed: u64, cost_usd: f64) -> Self {
        Self {
            event: "canceled",
            run_id: run_id.into(),
            runs_completed,
            cost_usd,
            timestamp: now_iso8601(),
        }
    }
}

/// `budget_cap` — emitted when the budget cap is reached.
#[derive(Debug, Serialize)]
pub struct BudgetCapJsonlEvent {
    pub event: &'static str,
    pub run_id: String,
    pub cost_usd: f64,
    pub budget_cap_usd: f64,
    pub runs_completed: u64,
    pub timestamp: String,
}

impl BudgetCapJsonlEvent {
    pub fn new(
        run_id: impl Into<String>,
        cost_usd: f64,
        budget_cap_usd: f64,
        runs_completed: u64,
    ) -> Self {
        Self {
            event: "budget_cap",
            run_id: run_id.into(),
            cost_usd,
            budget_cap_usd,
            runs_completed,
            timestamp: now_iso8601(),
        }
    }
}

/// `max_cycles` — emitted when the max cycle count is reached without completion.
#[derive(Debug, Serialize)]
pub struct MaxCyclesEvent {
    pub event: &'static str,
    pub run_id: String,
    pub cycles: u64,
    pub runs_completed: u64,
    pub cost_usd: f64,
    pub timestamp: String,
}

impl MaxCyclesEvent {
    pub fn new(run_id: impl Into<String>, cycles: u64, runs_completed: u64, cost_usd: f64) -> Self {
        Self {
            event: "max_cycles",
            run_id: run_id.into(),
            cycles,
            runs_completed,
            cost_usd,
            timestamp: now_iso8601(),
        }
    }
}

/// `delay_start` — emitted before a delay (inter-run, inter-cycle, or quota backoff).
#[derive(Debug, Serialize)]
pub struct DelayStartEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub cycle: u64,
    pub phase: String,
    pub delay_secs: u64,
    pub reason: String,
    pub timestamp: String,
}

impl DelayStartEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        run: u64,
        cycle: u64,
        phase: impl Into<String>,
        delay_secs: u64,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            event: "delay_start",
            run_id: run_id.into(),
            run,
            cycle,
            phase: phase.into(),
            delay_secs,
            reason: reason.into(),
            timestamp: now_iso8601(),
        }
    }
}

/// `delay_end` — emitted after a delay completes.
#[derive(Debug, Serialize)]
pub struct DelayEndEvent {
    pub event: &'static str,
    pub run_id: String,
    pub run: u64,
    pub timestamp: String,
}

impl DelayEndEvent {
    pub fn new(run_id: impl Into<String>, run: u64) -> Self {
        Self {
            event: "delay_end",
            run_id: run_id.into(),
            run,
            timestamp: now_iso8601(),
        }
    }
}

/// Per-phase summary information for the `summary` event.
#[derive(Debug, Serialize)]
pub struct PhaseSummary {
    pub name: String,
    pub runs: u64,
    pub cost_usd: f64,
}

/// `summary` — emitted at the end of a workflow run (all exit paths).
#[derive(Debug, Serialize)]
pub struct SummaryEvent {
    pub event: &'static str,
    pub run_id: String,
    pub status: String,
    pub cycles: u64,
    pub runs: u64,
    pub cost_usd: f64,
    pub duration_secs: f64,
    pub phases: Vec<PhaseSummary>,
    pub timestamp: String,
}

impl SummaryEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        run_id: impl Into<String>,
        status: impl Into<String>,
        cycles: u64,
        runs: u64,
        cost_usd: f64,
        duration_secs: f64,
        phases: Vec<PhaseSummary>,
    ) -> Self {
        Self {
            event: "summary",
            run_id: run_id.into(),
            status: status.into(),
            cycles,
            runs,
            cost_usd,
            duration_secs,
            phases,
            timestamp: now_iso8601(),
        }
    }
}

/// `fatal_error` — emitted when rings itself cannot continue.
/// `run_id` is `None` if the error occurred before a run ID was assigned.
#[derive(Debug, Serialize)]
pub struct FatalErrorEvent {
    pub event: &'static str,
    pub run_id: Option<String>,
    pub message: String,
    pub timestamp: String,
}

impl FatalErrorEvent {
    pub fn new(run_id: Option<String>, message: impl Into<String>) -> Self {
        Self {
            event: "fatal_error",
            run_id,
            message: message.into(),
            timestamp: now_iso8601(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_field(json: &str, key: &str) -> serde_json::Value {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v[key].clone()
    }

    #[test]
    fn test_start_event_serializes_correctly() {
        let ev = StartEvent::new("run_123", "my.rings.toml");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "start");
        assert_eq!(event_field(&json, "run_id"), "run_123");
        assert_eq!(event_field(&json, "schema_version"), 1);
        assert!(!event_field(&json, "timestamp").as_str().unwrap().is_empty());
    }

    #[test]
    fn test_run_start_event_serializes_correctly() {
        let ctx = serde_json::json!({"phase_name": "builder", "cycle": 1});
        let ev = RunStartEvent::new("run_123", 1, 1, "builder", 1, 3, ctx);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "run_start");
        assert_eq!(event_field(&json, "run_id"), "run_123");
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_run_end_event_serializes_correctly() {
        let ev = RunEndEvent::new(
            "run_123",
            1,
            1,
            "builder",
            1,
            Some(0.05),
            Some(1000),
            Some(200),
            0,
            vec![],
            "full",
            3,
            vec!["src/new.rs".to_string()],
            vec!["src/main.rs".to_string()],
            vec![],
            2,
        );
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "run_end");
        assert_eq!(event_field(&json, "run_id"), "run_123");
        assert!(event_field(&json, "timestamp").as_str().is_some());
        assert_eq!(event_field(&json, "files_changed"), 2u32);
        let files_added = serde_json::from_str::<serde_json::Value>(&json).unwrap();
        assert_eq!(
            files_added["files_added"],
            serde_json::json!(["src/new.rs"])
        );
        assert_eq!(
            files_added["files_modified"],
            serde_json::json!(["src/main.rs"])
        );
        assert_eq!(files_added["files_deleted"], serde_json::json!([]));
    }

    #[test]
    fn test_completion_signal_event_serializes_correctly() {
        let ev = CompletionSignalEvent::new("run_123", 7, 2, "builder", "TASK_COMPLETE");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "completion_signal");
        assert_eq!(event_field(&json, "signal"), "TASK_COMPLETE");
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_executor_error_event_serializes_correctly() {
        let ev = ExecutorErrorEvent::new("run_123", 7, 2, "builder", "quota", 1, "Usage limit");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "executor_error");
        assert_eq!(event_field(&json, "error_class"), "quota");
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_canceled_event_serializes_correctly() {
        let ev = CanceledEvent::new("run_123", 7, 1.42);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "canceled");
        assert_eq!(event_field(&json, "runs_completed"), 7);
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_budget_cap_event_serializes_correctly() {
        let ev = BudgetCapJsonlEvent::new("run_123", 5.03, 5.00, 42);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "budget_cap");
        assert_eq!(event_field(&json, "runs_completed"), 42);
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_max_cycles_event_serializes_correctly() {
        let ev = MaxCyclesEvent::new("run_123", 50, 200, 4.23);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "max_cycles");
        assert_eq!(event_field(&json, "cycles"), 50);
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_delay_start_event_serializes_correctly() {
        let ev = DelayStartEvent::new("run_123", 7, 2, "builder", 30, "inter_run");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "delay_start");
        assert_eq!(event_field(&json, "reason"), "inter_run");
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_delay_end_event_serializes_correctly() {
        let ev = DelayEndEvent::new("run_123", 7);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "delay_end");
        assert_eq!(event_field(&json, "run"), 7);
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_summary_event_serializes_correctly() {
        let phases = vec![
            PhaseSummary {
                name: "builder".to_string(),
                runs: 10,
                cost_usd: 0.89,
            },
            PhaseSummary {
                name: "reviewer".to_string(),
                runs: 2,
                cost_usd: 0.21,
            },
        ];
        let ev = SummaryEvent::new("run_123", "completed", 2, 12, 1.10, 494.0, phases);
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "summary");
        assert_eq!(event_field(&json, "status"), "completed");
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_fatal_error_event_run_id_null_when_none() {
        let ev = FatalErrorEvent::new(None, "Invalid workflow file");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "event"), "fatal_error");
        assert!(event_field(&json, "run_id").is_null());
        assert!(event_field(&json, "timestamp").as_str().is_some());
    }

    #[test]
    fn test_fatal_error_event_run_id_present_when_some() {
        let ev = FatalErrorEvent::new(Some("run_123".to_string()), "Some error");
        let json = serde_json::to_string(&ev).unwrap();
        assert_eq!(event_field(&json, "run_id"), "run_123");
    }

    #[test]
    fn test_run_id_and_timestamp_always_present() {
        // Check a sampling of event types
        let ev = StartEvent::new("run_abc", "wf.toml");
        let json = serde_json::to_string(&ev).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["run_id"].is_string());
        assert!(v["timestamp"].is_string());

        let ev = CanceledEvent::new("run_abc", 0, 0.0);
        let json = serde_json::to_string(&ev).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v["run_id"].is_string());
        assert!(v["timestamp"].is_string());
    }

    #[test]
    fn test_emit_jsonl_produces_valid_single_line_json() {
        // We test by serializing and checking no newlines embedded
        let ev = StartEvent::new("run_123", "wf.toml");
        let json = serde_json::to_string(&ev).unwrap();
        // Ensure no embedded newlines in the JSON itself
        assert!(!json.contains('\n'));
        // Parse back to confirm valid JSON
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }
}
