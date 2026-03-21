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
        return init_tracer_inner();
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
}
