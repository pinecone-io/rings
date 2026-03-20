use std::sync::atomic::{AtomicBool, Ordering};

use owo_colors::OwoColorize;

/// Braille spinner frames for animated status display.
pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Global color-enabled flag. Starts true; call `set_no_color()` to disable.
static COLOR_ENABLED: AtomicBool = AtomicBool::new(true);

/// Returns true if color output is currently enabled.
///
/// Color is disabled if:
/// - `set_no_color()` has been called (covers `--no-color` and non-TTY detection), or
/// - the `NO_COLOR` environment variable is set (per <https://no-color.org/>).
pub fn color_enabled() -> bool {
    COLOR_ENABLED.load(Ordering::Relaxed) && std::env::var_os("NO_COLOR").is_none()
}

/// Disable color output for this process.
pub fn set_no_color() {
    COLOR_ENABLED.store(false, Ordering::Relaxed);
}

/// Re-enable color output. Used in tests to reset global state.
#[cfg(any(test, feature = "testing"))]
pub fn set_color_enabled() {
    COLOR_ENABLED.store(true, Ordering::Relaxed);
}

/// Mutex to serialize tests that mutate global color state.
/// All tests that call `set_no_color()`, `set_color_enabled()`, or set/remove `NO_COLOR`
/// must hold this lock for the duration of the test to prevent interference.
#[cfg(any(test, feature = "testing"))]
pub static COLOR_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Returns the spinner frame for the given tick counter.
pub fn spinner_frame(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

/// Dim styling — chrome, labels, dividers.
pub fn dim(s: &str) -> String {
    if color_enabled() {
        s.dimmed().to_string()
    } else {
        s.to_string()
    }
}

/// Bold styling — emphasis: phase names, cycle numbers, key values.
pub fn bold(s: &str) -> String {
    if color_enabled() {
        s.bold().to_string()
    } else {
        s.to_string()
    }
}

/// Success styling — green for `✓` and completion text.
pub fn success(s: &str) -> String {
    if color_enabled() {
        s.bright_green().to_string()
    } else {
        s.to_string()
    }
}

/// Error styling — red for `✗` and error messages.
pub fn error(s: &str) -> String {
    if color_enabled() {
        s.bright_red().to_string()
    } else {
        s.to_string()
    }
}

/// Warning styling — yellow for `⚠` and advisory messages.
pub fn warn(s: &str) -> String {
    if color_enabled() {
        s.yellow().to_string()
    } else {
        s.to_string()
    }
}

/// Accent styling — cyan for cost figures and resume commands.
pub fn accent(s: &str) -> String {
    if color_enabled() {
        s.cyan().to_string()
    } else {
        s.to_string()
    }
}

/// Muted styling — dim for secondary info (paths, elapsed, audit locations).
pub fn muted(s: &str) -> String {
    if color_enabled() {
        s.dimmed().to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_enabled_respects_atomic_toggle() {
        let _guard = COLOR_TEST_LOCK.lock().unwrap();
        // Ensure we start with a known state
        set_color_enabled();
        std::env::remove_var("NO_COLOR");

        assert!(color_enabled(), "color should be enabled initially");
        set_no_color();
        assert!(
            !color_enabled(),
            "color should be disabled after set_no_color()"
        );

        // Reset for other tests
        set_color_enabled();
    }

    #[test]
    fn color_enabled_respects_no_color_env_var() {
        let _guard = COLOR_TEST_LOCK.lock().unwrap();
        set_color_enabled();
        std::env::remove_var("NO_COLOR");

        assert!(color_enabled(), "color should be enabled without NO_COLOR");
        std::env::set_var("NO_COLOR", "1");
        assert!(
            !color_enabled(),
            "color should be disabled when NO_COLOR is set"
        );

        std::env::remove_var("NO_COLOR");
        set_color_enabled();
    }

    #[test]
    fn spinner_frame_cycles_through_all_frames() {
        for i in 0..SPINNER_FRAMES.len() {
            assert_eq!(spinner_frame(i), SPINNER_FRAMES[i]);
        }
        // Verify wrapping
        assert_eq!(spinner_frame(SPINNER_FRAMES.len()), SPINNER_FRAMES[0]);
        assert_eq!(spinner_frame(SPINNER_FRAMES.len() + 3), SPINNER_FRAMES[3]);
    }

    #[test]
    fn helpers_return_unstyled_when_color_disabled() {
        let _guard = COLOR_TEST_LOCK.lock().unwrap();
        set_color_enabled();
        std::env::remove_var("NO_COLOR");
        set_no_color();

        let text = "hello";
        assert_eq!(dim(text), text);
        assert_eq!(bold(text), text);
        assert_eq!(success(text), text);
        assert_eq!(error(text), text);
        assert_eq!(warn(text), text);
        assert_eq!(accent(text), text);
        assert_eq!(muted(text), text);

        set_color_enabled();
    }

    #[test]
    fn helpers_return_ansi_styled_when_color_enabled() {
        let _guard = COLOR_TEST_LOCK.lock().unwrap();
        set_color_enabled();
        std::env::remove_var("NO_COLOR");

        let text = "hello";
        // When color is enabled, all helpers wrap the text in ANSI escape codes.
        assert_ne!(success(text), text, "success() should add ANSI codes");
        assert_ne!(error(text), text, "error() should add ANSI codes");
        assert_ne!(warn(text), text, "warn() should add ANSI codes");
        assert_ne!(accent(text), text, "accent() should add ANSI codes");
        assert_ne!(dim(text), text, "dim() should add ANSI codes");
        assert_ne!(muted(text), text, "muted() should add ANSI codes");
        assert_ne!(bold(text), text, "bold() should add ANSI codes");

        // Verify the styled strings actually contain the original text.
        assert!(success(text).contains(text));
        assert!(error(text).contains(text));
        assert!(warn(text).contains(text));
        assert!(accent(text).contains(text));
    }

    /// Verify that `set_no_color()` — used by main() for both `--no-color` flag and
    /// non-TTY detection — disables ANSI codes from all helpers.
    #[test]
    fn set_no_color_covers_both_no_color_flag_and_non_tty_detection() {
        let _guard = COLOR_TEST_LOCK.lock().unwrap();
        set_color_enabled();
        std::env::remove_var("NO_COLOR");

        // Both --no-color and non-TTY detection call set_no_color(), so this
        // test validates both code paths share the same mechanism.
        set_no_color();
        assert!(!color_enabled());

        let text = "hello";
        assert_eq!(success(text), text);
        assert_eq!(error(text), text);
        assert_eq!(warn(text), text);

        set_color_enabled();
    }
}
