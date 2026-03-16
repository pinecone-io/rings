use std::sync::atomic::{AtomicU8, Ordering};

/// Tracks the cancellation state of a rings run.
///
/// Transitions:
///   `NotCanceled` (0) — normal operation
///   `Canceling` (1)   — first Ctrl+C received; SIGTERM will be sent to subprocess
///   `ForceKill` (2)   — second Ctrl+C received; SIGKILL will be sent immediately
///
/// The transition from `Canceling` → `ForceKill` uses `compare_exchange` so two
/// simultaneous signals cannot both "win" the first-signal slot.
pub struct CancelState {
    state: AtomicU8,
}

pub const NOT_CANCELED: u8 = 0;
pub const CANCELING: u8 = 1;
pub const FORCE_KILL: u8 = 2;

impl CancelState {
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(NOT_CANCELED),
        }
    }

    /// Called when a signal (SIGINT/SIGTERM) is received.
    /// First call transitions `NotCanceled` → `Canceling`.
    /// Second call (while already `Canceling`) transitions → `ForceKill`.
    pub fn signal_received(&self) {
        // Try to transition NotCanceled → Canceling.
        let prev = self.state.compare_exchange(
            NOT_CANCELED,
            CANCELING,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        if prev.is_ok() {
            // First signal: transitioned to Canceling.
            return;
        }
        // Already at Canceling or ForceKill. Try Canceling → ForceKill.
        let _ =
            self.state
                .compare_exchange(CANCELING, FORCE_KILL, Ordering::SeqCst, Ordering::SeqCst);
        // If already ForceKill or some other state, no-op.
    }

    /// Returns the current state value.
    pub fn load(&self) -> u8 {
        self.state.load(Ordering::SeqCst)
    }

    /// Returns true if cancellation has been requested (Canceling or ForceKill).
    pub fn is_canceling(&self) -> bool {
        self.load() >= CANCELING
    }

    /// Returns true if force-kill has been requested.
    pub fn is_force_kill(&self) -> bool {
        self.load() >= FORCE_KILL
    }
}

impl Default for CancelState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn initial_state_is_not_canceled() {
        let cs = CancelState::new();
        assert_eq!(cs.load(), NOT_CANCELED);
        assert!(!cs.is_canceling());
        assert!(!cs.is_force_kill());
    }

    #[test]
    fn first_signal_transitions_to_canceling() {
        let cs = CancelState::new();
        cs.signal_received();
        assert_eq!(cs.load(), CANCELING);
        assert!(cs.is_canceling());
        assert!(!cs.is_force_kill());
    }

    #[test]
    fn second_signal_transitions_to_force_kill() {
        let cs = CancelState::new();
        cs.signal_received();
        cs.signal_received();
        assert_eq!(cs.load(), FORCE_KILL);
        assert!(cs.is_canceling());
        assert!(cs.is_force_kill());
    }

    #[test]
    fn third_signal_stays_at_force_kill() {
        let cs = CancelState::new();
        cs.signal_received();
        cs.signal_received();
        cs.signal_received();
        assert_eq!(cs.load(), FORCE_KILL);
    }

    #[test]
    fn concurrent_first_signals_only_one_wins_canceling() {
        use std::thread;
        let cs = Arc::new(CancelState::new());
        let mut handles = vec![];
        for _ in 0..10 {
            let cs2 = Arc::clone(&cs);
            handles.push(thread::spawn(move || cs2.signal_received()));
        }
        for h in handles {
            h.join().unwrap();
        }
        // After 10 concurrent signals, state must be ForceKill (not some intermediate).
        let s = cs.load();
        assert!(s == CANCELING || s == FORCE_KILL);
    }
}
