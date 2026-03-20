// Security: prompts go via stdin only (see CLAUDE.md and specs/execution/executor-integration.md).
// This test pins the exact arg list. Any PR that adds a runtime-content arg must update this test,
// making the security regression visible in review.
#[cfg(test)]
mod tests {
    use rings::executor::{ClaudeExecutor, ConfigurableExecutor, Executor, Invocation};

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
                "--output-format".to_string(),
                "stream-json".to_string(),
                "--verbose".to_string(),
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

    /// Verify that the executor subprocess inherits the parent environment.
    ///
    /// rings must NOT call env_clear() on the subprocess command — API keys and
    /// other credentials in the caller's environment must be visible to the executor.
    /// This test pins that invariant by setting a unique env var in the test process
    /// and asserting the spawned subprocess can read it.
    #[test]
    fn executor_inherits_parent_environment() {
        // Use a unique name to avoid collisions with other env vars.
        let var_name = "RINGS_TEST_ENV_PASSTHROUGH";
        let var_value = "rings_env_passthrough_ok";

        // Safety: test-only env mutation; tests that touch env vars should run
        // single-threaded (cargo test runs each integration test binary as its
        // own process, so this is safe here).
        unsafe { std::env::set_var(var_name, var_value) };

        let executor = ConfigurableExecutor {
            binary: "sh".to_string(),
            args: vec!["-c".to_string(), format!("printenv {var_name}")],
        };

        let tmp = std::env::temp_dir();
        let inv = Invocation {
            prompt: String::new(),
            context_dir: tmp,
        };

        let output = executor.run(&inv, false).expect("sh subprocess failed");
        // printenv exits 0 when the var is found; a missing var causes exit 1.
        assert_eq!(
            output.exit_code, 0,
            "printenv exited non-zero — env var not inherited"
        );
        assert!(
            output.combined.contains(var_value),
            "expected env var value '{var_value}' in subprocess output, got: {:?}",
            output.combined
        );

        unsafe { std::env::remove_var(var_name) };
    }
}
