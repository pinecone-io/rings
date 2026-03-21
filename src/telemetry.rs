//! OpenTelemetry integration for rings.
//!
//! This module is compiled when the `otel` feature flag is enabled (the default).
//! At runtime, OTel is a no-op unless `RINGS_OTEL_ENABLED=true` is set.
//!
//! Initialization failures are non-fatal: a warning is printed to stderr and
//! execution continues with a no-op tracer.

/// A handle to the initialized tracer provider.
///
/// Call `shutdown()` before process exit to flush any buffered spans.
pub struct TracerHandle {
    #[cfg(feature = "otel")]
    provider: Option<opentelemetry_sdk::trace::TracerProvider>,
    _private: (),
}

impl TracerHandle {
    /// Flush and shut down the tracer provider.
    pub fn shutdown(self) {
        #[cfg(feature = "otel")]
        if let Some(provider) = self.provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("Warning: OTel shutdown error: {e}");
            }
        }
    }
}

/// Initialize OpenTelemetry tracing.
///
/// Returns a `TracerHandle` that should be shut down before process exit.
///
/// If `RINGS_OTEL_ENABLED` is not `"true"`, returns a no-op handle immediately.
/// If initialization fails, prints a warning to stderr and returns a no-op handle.
pub fn init_tracer() -> TracerHandle {
    #[cfg(feature = "otel")]
    {
        init_tracer_inner()
    }
    #[cfg(not(feature = "otel"))]
    {
        TracerHandle { _private: () }
    }
}

#[cfg(feature = "otel")]
fn init_tracer_inner() -> TracerHandle {
    if std::env::var("RINGS_OTEL_ENABLED").unwrap_or_default() != "true" {
        return TracerHandle {
            provider: None,
            _private: (),
        };
    }

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4317".to_string());

    match build_provider(&endpoint) {
        Ok(provider) => {
            opentelemetry::global::set_tracer_provider(provider.clone());
            TracerHandle {
                provider: Some(provider),
                _private: (),
            }
        }
        Err(e) => {
            eprintln!("Warning: OTel initialization failed: {e}. Continuing without telemetry.");
            TracerHandle {
                provider: None,
                _private: (),
            }
        }
    }
}

#[cfg(feature = "otel")]
fn build_provider(endpoint: &str) -> anyhow::Result<opentelemetry_sdk::trace::TracerProvider> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::Resource;

    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint)
        .build_span_exporter()
        .map_err(|e| anyhow::anyhow!("failed to build OTLP span exporter: {e}"))?;

    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "rings".to_string());

    let resource = Resource::new(vec![
        KeyValue::new("service.name", service_name),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION").to_string()),
    ]);

    let config = opentelemetry_sdk::trace::Config::default().with_resource(resource);

    let provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_config(config)
        .build();

    Ok(provider)
}

// ─── Span management ────────────────────────────────────────────────────────

/// Attributes for a single phase-run span.
///
/// Constructed in the engine loop and passed to [`RunTracer::record_phase_run`].
pub struct PhaseRunData {
    pub run_id: String,
    pub workflow_file: String,
    pub cycle: u32,
    pub phase_name: String,
    pub iteration: u32,
    pub total_iterations: u32,
    pub global_run_number: u32,
    pub exit_code: i32,
    pub completion_signal_found: bool,
    pub files_changed: u32,
    pub cost_usd: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub start_time: std::time::SystemTime,
    pub end_time: std::time::SystemTime,
}

/// Manages the OTel span hierarchy for one `rings run` invocation.
///
/// Holds the root `rings.run` span and the current `rings.cycle` span.
/// Both spans are ended (and flushed) when this struct is dropped.
/// All methods are no-ops when `RINGS_OTEL_ENABLED != "true"` or the `otel`
/// feature is disabled.
pub struct RunTracer {
    #[cfg(feature = "otel")]
    inner: Option<RunTracerInner>,
    _private: (),
}

#[cfg(feature = "otel")]
struct RunTracerInner {
    // Fields drop in declaration order: cycle_cx drops before run_cx,
    // ensuring the child cycle span ends before the root run span.
    cycle_cx: Option<opentelemetry::Context>,
    run_cx: opentelemetry::Context,
    include_tokens: bool,
    /// Hex trace ID of this run's root span (stored for writing to run.toml).
    trace_id_hex: String,
    /// Hex span ID of this run's root span (stored for writing to run.toml).
    span_id_hex: String,
}

#[cfg(feature = "otel")]
impl Drop for RunTracerInner {
    fn drop(&mut self) {
        use opentelemetry::trace::TraceContextExt;
        // End cycle span before run span.
        if let Some(ref cycle_cx) = self.cycle_cx {
            cycle_cx.span().end();
        }
        self.run_cx.span().end();
    }
}

impl RunTracer {
    /// Create a new `RunTracer` and open the root `rings.run` span.
    ///
    /// `parent_link` optionally carries `(parent_run_id, trace_id_hex, span_id_hex)` from a
    /// prior run. When present, the root span gets a W3C span link to the parent run's root span
    /// plus a `rings.parent_run_id` attribute.  If the hex values are invalid or the feature is
    /// disabled, the link is silently omitted — OTel is never fatal.
    ///
    /// Returns a no-op tracer if OTel is disabled.
    pub fn new(
        run_id: &str,
        workflow_file: &str,
        max_cycles: u32,
        phases: &str,
        parent_link: Option<(&str, &str, &str)>,
    ) -> Self {
        #[cfg(feature = "otel")]
        {
            if std::env::var("RINGS_OTEL_ENABLED").unwrap_or_default() != "true" {
                return RunTracer {
                    inner: None,
                    _private: (),
                };
            }
            use opentelemetry::trace::{
                Span, SpanContext, TraceContextExt, TraceFlags, TraceState, Tracer,
            };
            use opentelemetry::{global, Context, KeyValue};

            let include_tokens =
                std::env::var("RINGS_OTEL_INCLUDE_TOKENS").unwrap_or_default() == "true";

            let tracer = global::tracer("rings");

            // Build the root span, optionally attaching a span link to the parent run.
            let mut builder = tracer.span_builder("rings.run");
            if let Some((_, trace_hex, span_hex)) = parent_link {
                if let (Ok(trace_id), Ok(span_id)) = (
                    opentelemetry::trace::TraceId::from_hex(trace_hex),
                    opentelemetry::trace::SpanId::from_hex(span_hex),
                ) {
                    let parent_span_ctx = SpanContext::new(
                        trace_id,
                        span_id,
                        TraceFlags::SAMPLED,
                        true, // is_remote: the parent span is from a different process invocation
                        TraceState::NONE,
                    );
                    builder = builder.with_links(vec![opentelemetry::trace::Link::with_context(
                        parent_span_ctx,
                    )]);
                }
            }
            let mut run_span = builder.start(&tracer);

            run_span.set_attribute(KeyValue::new("rings.run.id", run_id.to_string()));
            run_span.set_attribute(KeyValue::new(
                "rings.workflow.file",
                workflow_file.to_string(),
            ));
            run_span.set_attribute(KeyValue::new("max_cycles", max_cycles as i64));
            run_span.set_attribute(KeyValue::new("phases", phases.to_string()));
            if let Some((parent_run_id, _, _)) = parent_link {
                run_span.set_attribute(KeyValue::new(
                    "rings.parent_run_id",
                    parent_run_id.to_string(),
                ));
            }

            // Capture trace/span IDs before moving the span into the context.
            let span_ctx = run_span.span_context();
            let trace_id_hex = format!("{}", span_ctx.trace_id());
            let span_id_hex = format!("{}", span_ctx.span_id());

            let run_cx = Context::current_with_span(run_span);

            RunTracer {
                inner: Some(RunTracerInner {
                    cycle_cx: None,
                    run_cx,
                    include_tokens,
                    trace_id_hex,
                    span_id_hex,
                }),
                _private: (),
            }
        }
        #[cfg(not(feature = "otel"))]
        RunTracer { _private: () }
    }

    /// Return the hex trace ID and span ID of this run's root span.
    ///
    /// Returns `None` when OTel is disabled or the tracer is a no-op.
    pub fn get_trace_context(&self) -> Option<(String, String)> {
        #[cfg(feature = "otel")]
        if let Some(inner) = &self.inner {
            return Some((inner.trace_id_hex.clone(), inner.span_id_hex.clone()));
        }
        None
    }

    /// End the previous cycle span (if any) and start a new `rings.cycle` span.
    pub fn start_cycle(&mut self, cycle_number: u32) {
        #[cfg(feature = "otel")]
        if let Some(inner) = &mut self.inner {
            use opentelemetry::trace::{Span, TraceContextExt, Tracer};
            use opentelemetry::KeyValue;

            // End previous cycle span.
            if let Some(ref prev_cx) = inner.cycle_cx {
                prev_cx.span().end();
            }

            let tracer = opentelemetry::global::tracer("rings");
            let mut cycle_span = tracer.start_with_context("rings.cycle", &inner.run_cx);
            cycle_span.set_attribute(KeyValue::new("cycle.number", cycle_number as i64));
            inner.cycle_cx = Some(inner.run_cx.with_span(cycle_span));
        }
    }

    /// Record a completed phase-run span with explicit start/end times.
    ///
    /// The span is immediately ended after all attributes are set.
    pub fn record_phase_run(&self, data: &PhaseRunData) {
        #[cfg(feature = "otel")]
        if let Some(inner) = &self.inner {
            use opentelemetry::trace::{Span, Status, Tracer};
            use opentelemetry::KeyValue;

            let parent_cx = inner.cycle_cx.as_ref().unwrap_or(&inner.run_cx);
            let tracer = opentelemetry::global::tracer("rings");
            let mut span = tracer
                .span_builder("rings.phase.run")
                .with_start_time(data.start_time)
                .start_with_context(&tracer, parent_cx);

            span.set_attribute(KeyValue::new("rings.run.id", data.run_id.clone()));
            span.set_attribute(KeyValue::new(
                "rings.workflow.file",
                data.workflow_file.clone(),
            ));
            span.set_attribute(KeyValue::new("rings.cycle", data.cycle as i64));
            span.set_attribute(KeyValue::new("rings.phase.name", data.phase_name.clone()));
            span.set_attribute(KeyValue::new(
                "rings.phase.iteration",
                data.iteration as i64,
            ));
            span.set_attribute(KeyValue::new(
                "rings.phase.total_iterations",
                data.total_iterations as i64,
            ));
            span.set_attribute(KeyValue::new(
                "rings.run.global_number",
                data.global_run_number as i64,
            ));
            span.set_attribute(KeyValue::new("rings.exit_code", data.exit_code as i64));
            span.set_attribute(KeyValue::new(
                "rings.completion_signal_found",
                data.completion_signal_found,
            ));
            span.set_attribute(KeyValue::new(
                "rings.files_changed",
                data.files_changed as i64,
            ));

            if inner.include_tokens {
                if let Some(cost) = data.cost_usd {
                    span.set_attribute(KeyValue::new("rings.cost.usd", cost));
                }
                if let Some(tokens) = data.input_tokens {
                    span.set_attribute(KeyValue::new("rings.tokens.input", tokens as i64));
                }
                if let Some(tokens) = data.output_tokens {
                    span.set_attribute(KeyValue::new("rings.tokens.output", tokens as i64));
                }
            }

            if data.exit_code != 0 {
                span.set_status(Status::error(format!("exit code {}", data.exit_code)));
            } else {
                span.set_status(Status::Ok);
            }

            if data.completion_signal_found {
                span.add_event("rings.completion_signal", vec![]);
            }

            span.end_with_timestamp(data.end_time);
        }
    }

    /// Set the `rings.status` attribute and span status on the root `rings.run` span.
    ///
    /// `is_error` maps to OTel `Status::Error`; otherwise `Status::Ok` is used.
    pub fn set_run_status(&self, status: &str, is_error: bool) {
        #[cfg(feature = "otel")]
        if let Some(inner) = &self.inner {
            use opentelemetry::trace::{Status, TraceContextExt};
            use opentelemetry::KeyValue;

            inner
                .run_cx
                .span()
                .set_attribute(KeyValue::new("rings.status", status.to_string()));
            if is_error {
                inner
                    .run_cx
                    .span()
                    .set_status(Status::error(status.to_string()));
            } else {
                inner.run_cx.span().set_status(Status::Ok);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default_returns_noop() {
        // Without RINGS_OTEL_ENABLED=true, init_tracer returns a no-op handle.
        std::env::remove_var("RINGS_OTEL_ENABLED");
        let handle = init_tracer();
        #[cfg(feature = "otel")]
        assert!(handle.provider.is_none());
        handle.shutdown();
    }

    #[test]
    fn disabled_when_set_to_false() {
        std::env::set_var("RINGS_OTEL_ENABLED", "false");
        let handle = init_tracer();
        #[cfg(feature = "otel")]
        assert!(handle.provider.is_none());
        handle.shutdown();
        std::env::remove_var("RINGS_OTEL_ENABLED");
    }

    #[test]
    fn disabled_when_set_to_one() {
        // "1" is not "true" — should be disabled per spec.
        std::env::set_var("RINGS_OTEL_ENABLED", "1");
        let handle = init_tracer();
        #[cfg(feature = "otel")]
        assert!(handle.provider.is_none());
        handle.shutdown();
        std::env::remove_var("RINGS_OTEL_ENABLED");
    }

    #[test]
    #[cfg(feature = "otel")]
    fn enabled_with_unreachable_endpoint_falls_back_to_noop() {
        // Even if endpoint is unreachable, init should succeed (connection happens at export time).
        std::env::set_var("RINGS_OTEL_ENABLED", "true");
        std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:1");
        let handle = init_tracer();
        // May succeed (provider created) or fail (if URL is rejected early); either way no panic.
        handle.shutdown();
        std::env::remove_var("RINGS_OTEL_ENABLED");
        std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
    }

    #[test]
    #[cfg(feature = "otel")]
    fn shutdown_is_safe_with_no_provider() {
        let handle = TracerHandle {
            provider: None,
            _private: (),
        };
        handle.shutdown(); // Must not panic.
    }

    #[test]
    fn run_tracer_noop_when_disabled() {
        std::env::remove_var("RINGS_OTEL_ENABLED");
        let mut tracer = RunTracer::new(
            "run_001",
            "/path/to/workflow.toml",
            5,
            "builder,reviewer",
            None,
        );
        tracer.start_cycle(1);
        tracer.record_phase_run(&PhaseRunData {
            run_id: "run_001".to_string(),
            workflow_file: "/path/to/workflow.toml".to_string(),
            cycle: 1,
            phase_name: "builder".to_string(),
            iteration: 1,
            total_iterations: 1,
            global_run_number: 1,
            exit_code: 0,
            completion_signal_found: false,
            files_changed: 3,
            cost_usd: Some(0.05),
            input_tokens: Some(1000),
            output_tokens: Some(500),
            start_time: std::time::SystemTime::now(),
            end_time: std::time::SystemTime::now(),
        });
        tracer.set_run_status("completed", false);
        // No panic — no-op path.
    }

    #[test]
    #[cfg(feature = "otel")]
    fn run_tracer_noop_when_otel_disabled_at_runtime() {
        std::env::set_var("RINGS_OTEL_ENABLED", "false");
        let mut tracer = RunTracer::new("run_002", "/wf.toml", 3, "phase1", None);
        tracer.start_cycle(1);
        tracer.set_run_status("max_cycles", false);
        drop(tracer); // Must not panic.
        std::env::remove_var("RINGS_OTEL_ENABLED");
    }

    #[test]
    fn fresh_run_has_no_parent_link_and_get_trace_context_is_none_when_disabled() {
        // When OTel is disabled, get_trace_context() returns None for any run.
        std::env::remove_var("RINGS_OTEL_ENABLED");
        let tracer = RunTracer::new("run_fresh", "/wf.toml", 3, "phase1", None);
        assert!(
            tracer.get_trace_context().is_none(),
            "disabled OTel should return None for trace context"
        );
    }

    #[test]
    fn missing_parent_trace_id_handled_gracefully() {
        // Passing None parent link must not panic.
        std::env::remove_var("RINGS_OTEL_ENABLED");
        let tracer = RunTracer::new("run_child", "/wf.toml", 3, "phase1", None);
        assert!(tracer.get_trace_context().is_none());
    }

    #[test]
    fn invalid_parent_hex_handled_gracefully() {
        // Passing garbage hex values must not panic; link is silently dropped.
        std::env::remove_var("RINGS_OTEL_ENABLED");
        let tracer = RunTracer::new(
            "run_child",
            "/wf.toml",
            3,
            "phase1",
            Some(("run_parent", "not_a_trace_id", "not_a_span_id")),
        );
        // No panic. In disabled mode, always None.
        assert!(tracer.get_trace_context().is_none());
    }
}
