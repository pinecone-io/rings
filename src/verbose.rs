use crate::style;
use serde_json::Value;

/// Parse a stream-json line and render a human-friendly summary.
///
/// Returns `Some(text)` to display, or `None` to suppress the line.
/// Non-JSON input or unknown event types are passed through as-is.
pub fn format_stream_event(line: &str) -> Option<String> {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        // Non-JSON — pass through unchanged (graceful fallback for custom executors)
        return Some(line.to_string());
    };

    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");

    match event_type {
        "system" => None,           // suppress — contains internal metadata
        "rate_limit_event" => None, // suppress — internal bookkeeping
        "result" => None,           // suppress — rings shows its own summary

        "assistant" => {
            let content = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_array);

            let Some(blocks) = content else {
                // Unknown assistant shape — pass through
                return Some(line.to_string());
            };

            let mut parts: Vec<String> = Vec::new();
            for block in blocks {
                let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            parts.push(text.to_string());
                        }
                    }
                    "tool_use" => {
                        let name = block
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        let summary = format_tool_use(name, block.get("input"));
                        parts.push(style::dim(&summary));
                    }
                    _ => {}
                }
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }

        "user" => {
            // Tool results — show abbreviated line count
            let content = value
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_array);

            let Some(blocks) = content else {
                return Some(line.to_string());
            };

            let mut parts: Vec<String> = Vec::new();
            for block in blocks {
                let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                if block_type == "tool_result" {
                    let n_lines = count_result_lines(block.get("content"));
                    let summary = format!("  [tool result: {} lines]", n_lines);
                    parts.push(style::dim(&summary));
                }
            }

            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }

        _ if event_type.is_empty() => {
            // Not a recognized event shape — pass through
            Some(line.to_string())
        }

        _ => {
            // Unknown event type — pass through as-is
            Some(line.to_string())
        }
    }
}

/// Format a tool_use block into a one-line summary: `  Tool: <name>  key=value ...`
fn format_tool_use(name: &str, input: Option<&Value>) -> String {
    let mut s = format!("  Tool: {}", name);
    if let Some(Value::Object(map)) = input {
        for (k, v) in map.iter().take(3) {
            let v_str = match v {
                Value::String(sv) => {
                    // Truncate long strings
                    if sv.len() > 60 {
                        format!("{}…", &sv[..60])
                    } else {
                        sv.clone()
                    }
                }
                other => other.to_string(),
            };
            s.push_str(&format!("  {}={}", k, v_str));
        }
    }
    s
}

/// Count lines in a tool result content value.
fn count_result_lines(content: Option<&Value>) -> usize {
    match content {
        Some(Value::String(s)) => s.lines().count().max(1),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|item| {
                item.get("text")
                    .and_then(Value::as_str)
                    .map(|t| t.lines().count().max(1))
                    .unwrap_or(1)
            })
            .sum::<usize>()
            .max(1),
        Some(other) => {
            let s = other.to_string();
            s.lines().count().max(1)
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_init_returns_none() {
        let line = r#"{"type":"system","subtype":"init","cwd":"/foo","tools":[]}"#;
        assert_eq!(format_stream_event(line), None);
    }

    #[test]
    fn rate_limit_event_returns_none() {
        let line = r#"{"type":"rate_limit_event","rate_limit_info":{}}"#;
        assert_eq!(format_stream_event(line), None);
    }

    #[test]
    fn result_event_returns_none() {
        let line = r#"{"type":"result","subtype":"success","total_cost_usd":0.01,"result":"done"}"#;
        assert_eq!(format_stream_event(line), None);
    }

    #[test]
    fn assistant_text_event_returns_text() {
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello world"}]}}"#;
        let result = format_stream_event(line);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn assistant_tool_use_event_returns_summary() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/foo/bar.rs"}}]}}"#;
        let result = format_stream_event(line);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Tool: Read"));
        assert!(text.contains("file_path"));
        assert!(text.contains("/foo/bar.rs"));
    }

    #[test]
    fn assistant_mixed_content_renders_both() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me read that"},{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#;
        let result = format_stream_event(line);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("Let me read that"));
        assert!(text.contains("Tool: Read"));
    }

    #[test]
    fn user_tool_result_returns_line_count() {
        // Use JSON-escaped newlines so the string remains valid JSON
        let line = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"line1\nline2\nline3"}]}}"#;
        let result = format_stream_event(line);
        assert!(result.is_some());
        let text = result.unwrap();
        assert!(text.contains("[tool result:"));
        assert!(text.contains("lines]") || text.contains("line]"));
    }

    #[test]
    fn non_json_input_returns_line_as_is() {
        let line = "This is not JSON at all";
        let result = format_stream_event(line);
        assert_eq!(result, Some(line.to_string()));
    }

    #[test]
    fn unknown_json_event_type_returns_line_as_is() {
        let line = r#"{"type":"some_future_event","data":"value"}"#;
        let result = format_stream_event(line);
        assert_eq!(result, Some(line.to_string()));
    }
}
