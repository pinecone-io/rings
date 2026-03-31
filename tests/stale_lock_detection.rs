use rings::lock::{ContextLock, LockFile};
use tempfile::TempDir;

#[test]
fn stale_removed_is_some_when_stale_lock_removed() {
    let tmp = TempDir::new().unwrap();
    let lock_file = tmp.path().join(".rings.lock");

    // Create a stale lock with an invalid PID
    let stale_lock = LockFile {
        run_id: "old_run".to_string(),
        pid: 0,
    };
    let json = serde_json::to_string(&stale_lock).unwrap();
    std::fs::write(&lock_file, json).unwrap();

    // Acquire should succeed and populate stale_removed
    let result = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap();

    assert!(result.stale_removed.is_some());
    let stale_info = result.stale_removed.unwrap();
    assert_eq!(stale_info.run_id, "old_run");
    assert_eq!(stale_info.pid, 0);
}

#[test]
fn stale_removed_is_none_when_no_stale_lock_existed() {
    let tmp = TempDir::new().unwrap();

    // Acquire lock when no prior lock exists
    let result = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap();

    assert!(result.stale_removed.is_none());
}

#[test]
fn active_pid_lock_still_returns_error() {
    let tmp = TempDir::new().unwrap();
    let lock_file = tmp.path().join(".rings.lock");

    // Create a lock with our own PID (which is alive)
    let active_lock = LockFile {
        run_id: "active_run".to_string(),
        pid: std::process::id(),
    };
    let json = serde_json::to_string(&active_lock).unwrap();
    std::fs::write(&lock_file, json).unwrap();

    // Try to acquire without force should fail
    let err = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap_err();

    match err {
        rings::lock::LockError::ActiveProcess { run_id, pid, .. } => {
            assert_eq!(run_id, "active_run");
            assert_eq!(pid, std::process::id());
        }
        _ => panic!("Expected ActiveProcess error"),
    }
}

#[test]
fn force_lock_still_bypasses_check() {
    let tmp = TempDir::new().unwrap();
    let lock_file = tmp.path().join(".rings.lock");

    // Create a lock with our own PID (which is alive)
    let active_lock = LockFile {
        run_id: "active_run".to_string(),
        pid: std::process::id(),
    };
    let json = serde_json::to_string(&active_lock).unwrap();
    std::fs::write(&lock_file, json).unwrap();

    // With force=true, should succeed
    let result = ContextLock::acquire(tmp.path(), "new_run", true, None).unwrap();

    // Should overwrite without reporting it as stale (force doesn't go through stale detection)
    assert!(result.stale_removed.is_none());

    let new_contents = std::fs::read_to_string(&lock_file).unwrap();
    let parsed: LockFile = serde_json::from_str(&new_contents).unwrap();
    assert_eq!(parsed.run_id, "new_run");
}

#[test]
fn stale_lock_info_contains_correct_run_id_and_pid() {
    let tmp = TempDir::new().unwrap();
    let lock_file = tmp.path().join(".rings.lock");

    // Create a stale lock with specific values
    let stale_lock = LockFile {
        run_id: "run_20240115_abc123".to_string(),
        pid: 12345,
    };
    let json = serde_json::to_string(&stale_lock).unwrap();
    std::fs::write(&lock_file, json).unwrap();

    let result = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap();

    let stale_info = result.stale_removed.unwrap();
    assert_eq!(stale_info.run_id, "run_20240115_abc123");
    assert_eq!(stale_info.pid, 12345);
}
