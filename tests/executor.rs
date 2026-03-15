// Test file for executor module
// Tests will be moved inline in src/executor.rs after compilation
#[cfg(test)]
mod tests {
    use rings::executor::ClaudeExecutor;

    #[test]
    fn claude_executor_never_puts_prompt_in_args() {
        // Build args must not accept a prompt parameter at all.
        // The fixed arg list must not contain any user-controlled content.
        let args = ClaudeExecutor::build_args();
        // Verify the fixed args are exactly what we expect — any addition
        // that accepts user content would be a security regression.
        assert_eq!(
            args,
            vec![
                "--dangerously-skip-permissions".to_string(),
                "-p".to_string(),
                "-".to_string(),
            ]
        );
        // Verify no arg is a template that could accept runtime content.
        for arg in &args {
            assert!(
                !arg.contains('{'),
                "arg contains template placeholder: {arg}"
            );
            assert!(!arg.contains('%'), "arg contains format placeholder: {arg}");
        }
    }
}
