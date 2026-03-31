#![cfg(unix)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub run_id: String,
    pub pid: u32,
}

#[derive(Debug, Clone)]
pub struct StaleLockInfo {
    pub run_id: String,
    pub pid: u32,
}

#[derive(Debug)]
pub struct LockAcquireResult {
    pub lock: ContextLock,
    pub stale_removed: Option<StaleLockInfo>,
}

#[derive(Debug)]
pub enum LockError {
    ActiveProcess {
        run_id: String,
        pid: u32,
        context_dir: PathBuf,
        lock_name: Option<String>,
    },
    ContextDirMissing {
        path: PathBuf,
    },
    Io(std::io::Error),
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::ActiveProcess {
                run_id,
                pid,
                context_dir,
                lock_name,
            } => {
                if let Some(name) = lock_name {
                    write!(
                        f,
                        "Error: Another rings run ({run_id}, PID={pid}) holds lock \"{name}\" on {}.\nWait for it to finish or use --force-lock to override.",
                        context_dir.display()
                    )
                } else {
                    write!(
                        f,
                        "Error: Another rings run ({run_id}, PID={pid}) is already using {}.\nWait for it to finish or use --force-lock to override.",
                        context_dir.display()
                    )
                }
            }
            LockError::ContextDirMissing { path } => {
                write!(f, "context_dir does not exist: {}", path.display())
            }
            LockError::Io(e) => write!(f, "failed to access lock file: {e}"),
        }
    }
}

impl std::error::Error for LockError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LockError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LockError {
    fn from(e: std::io::Error) -> Self {
        LockError::Io(e)
    }
}

impl From<serde_json::Error> for LockError {
    fn from(e: serde_json::Error) -> Self {
        LockError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
    }
}

/// Returns the lock file path for the given context_dir and optional lock name.
///
/// - `None` → `<context_dir>/.rings.lock`
/// - `Some("planner")` → `<context_dir>/.rings.lock.planner`
fn lock_file_path(context_dir: &Path, lock_name: Option<&str>) -> PathBuf {
    match lock_name {
        None => context_dir.join(".rings.lock"),
        Some(name) => context_dir.join(format!(".rings.lock.{name}")),
    }
}

/// A lock file for the context directory. Automatically removes the lock file
/// when dropped (unless an error occurs).
#[derive(Debug)]
pub struct ContextLock {
    path: PathBuf,
}

impl ContextLock {
    /// Acquire a lock for the context directory.
    ///
    /// `lock_name` determines which lock file to use:
    /// - `None` → `.rings.lock` (default, unnamed lock)
    /// - `Some("planner")` → `.rings.lock.planner` (named lock)
    ///
    /// Returns `Err(LockError)` if:
    /// - The context_dir does not exist
    /// - Another active process holds the lock
    /// - `force` is false and a stale lock exists after removal attempt
    ///
    /// If `force` is true, overwrites any existing lock file.
    pub fn acquire(
        context_dir: impl AsRef<Path>,
        run_id: impl AsRef<str>,
        force: bool,
        lock_name: Option<&str>,
    ) -> Result<LockAcquireResult, LockError> {
        let context_dir = context_dir.as_ref();
        let run_id = run_id.as_ref();

        // Check context_dir exists
        if !context_dir.is_dir() {
            return Err(LockError::ContextDirMissing {
                path: context_dir.to_path_buf(),
            });
        }

        let path = lock_file_path(context_dir, lock_name);
        let lock_data = LockFile {
            run_id: run_id.to_string(),
            pid: std::process::id(),
        };
        let lock_json = serde_json::to_string(&lock_data)
            .map_err(|e| LockError::Io(std::io::Error::other(e)))?;

        if force {
            // Force overwrite
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)?;
            f.write_all(lock_json.as_bytes())?;
            return Ok(LockAcquireResult {
                lock: ContextLock { path },
                stale_removed: None,
            });
        }

        // Attempt atomic write with O_CREAT|O_EXCL
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(lock_json.as_bytes())?;
                Ok(LockAcquireResult {
                    lock: ContextLock { path },
                    stale_removed: None,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lock file exists. Check if it's stale.
                let mut stale_info: Option<StaleLockInfo> = None;
                if let Ok(existing_lock) = Self::read_lock_file(&path) {
                    // Check if the pid in the lock file is alive
                    if Self::is_process_alive(existing_lock.pid) {
                        return Err(LockError::ActiveProcess {
                            run_id: existing_lock.run_id,
                            pid: existing_lock.pid,
                            context_dir: context_dir.to_path_buf(),
                            lock_name: lock_name.map(str::to_string),
                        });
                    }
                    // Lock is stale, remember it
                    stale_info = Some(StaleLockInfo {
                        run_id: existing_lock.run_id,
                        pid: existing_lock.pid,
                    });
                }
                // Lock is stale or unreadable. Try to remove and retry once.
                let _ = std::fs::remove_file(&path);

                // Second attempt
                match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(mut f) => {
                        f.write_all(lock_json.as_bytes())?;
                        Ok(LockAcquireResult {
                            lock: ContextLock { path },
                            stale_removed: stale_info,
                        })
                    }
                    Err(_) => {
                        // Second attempt failed, probably another process got it.
                        // Try to read it again to give a better error.
                        if let Ok(existing_lock) = Self::read_lock_file(&path) {
                            Err(LockError::ActiveProcess {
                                run_id: existing_lock.run_id,
                                pid: existing_lock.pid,
                                context_dir: context_dir.to_path_buf(),
                                lock_name: lock_name.map(str::to_string),
                            })
                        } else {
                            // Couldn't read, just report ActiveProcess with a placeholder
                            Err(LockError::ActiveProcess {
                                run_id: "unknown".to_string(),
                                pid: 0,
                                context_dir: context_dir.to_path_buf(),
                                lock_name: lock_name.map(str::to_string),
                            })
                        }
                    }
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Read and parse a lock file, treating empty files and parse errors as missing locks.
    fn read_lock_file(path: &Path) -> Result<LockFile, Box<dyn std::error::Error>> {
        let mut f = File::open(path)?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;

        if contents.trim().is_empty() {
            return Err("empty lock file".into());
        }

        let lock_data: LockFile = serde_json::from_str(&contents)?;
        Ok(lock_data)
    }

    /// Check if a process with the given PID is alive using `kill(pid, 0)`.
    /// Returns true if the process is alive (EPERM or OK).
    /// Returns false if the process is not alive (ESRCH) or if pid is 0 (invalid/stale).
    fn is_process_alive(pid: u32) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        // pid=0 is invalid and treated as stale
        if pid == 0 {
            return false;
        }

        let pid = Pid::from_raw(pid as i32);
        match kill(pid, None) {
            Ok(()) => true,                  // Process is alive
            Err(nix::Error::EPERM) => true,  // Process is alive but we don't have permission
            Err(nix::Error::ESRCH) => false, // Process does not exist
            Err(_) => true,                  // Assume alive on other errors to be conservative
        }
    }
}

impl Drop for ContextLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_lock_acquire_success() {
        let tmp = TempDir::new().unwrap();
        let result = ContextLock::acquire(tmp.path(), "test_run", false, None).unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        assert!(lock_file.exists());
        assert!(result.stale_removed.is_none());
        drop(result.lock);
        assert!(!lock_file.exists());
    }

    #[test]
    fn test_lock_file_format() {
        let tmp = TempDir::new().unwrap();
        let result = ContextLock::acquire(tmp.path(), "test_run_123", false, None).unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        let contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.run_id, "test_run_123");
        assert!(parsed.pid > 0);
        drop(result.lock);
    }

    #[test]
    fn test_context_dir_missing() {
        let err = ContextLock::acquire("/nonexistent/path", "test_run", false, None).unwrap_err();
        match err {
            LockError::ContextDirMissing { .. } => {}
            _ => panic!("expected ContextDirMissing"),
        }
    }

    #[test]
    fn test_stale_lock_removed() {
        let tmp = TempDir::new().unwrap();
        // Create a stale lock with pid=0 (which is not a valid process)
        let lock_file = tmp.path().join(".rings.lock");
        let stale_lock = LockFile {
            run_id: "old_run".to_string(),
            pid: 0,
        };
        let json = serde_json::to_string(&stale_lock).unwrap();
        std::fs::write(&lock_file, json).unwrap();

        // Acquiring should succeed and remove the stale lock
        let result = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap();
        assert_eq!(result.lock.path, lock_file);
        assert!(result.stale_removed.is_some());
        assert_eq!(result.stale_removed.as_ref().unwrap().run_id, "old_run");
        assert_eq!(result.stale_removed.as_ref().unwrap().pid, 0);

        let new_contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&new_contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
        drop(result.lock);
    }

    #[test]
    fn test_empty_lock_file_treated_as_stale() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        std::fs::write(&lock_file, "").unwrap();

        // Should succeed and write a new lock. Empty file is unreadable, so no stale info.
        let result = ContextLock::acquire(tmp.path(), "new_run", false, None).unwrap();
        assert!(result.stale_removed.is_none());
        let contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
        drop(result.lock);
    }

    #[test]
    fn test_force_lock() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        let old_lock = LockFile {
            run_id: "old_run".to_string(),
            pid: std::process::id(),
        };
        let json = serde_json::to_string(&old_lock).unwrap();
        std::fs::write(&lock_file, json).unwrap();

        // With force=true, should overwrite even though our own process is "alive"
        let result = ContextLock::acquire(tmp.path(), "new_run", true, None).unwrap();
        assert!(result.stale_removed.is_none());
        let new_contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&new_contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
        drop(result.lock);
    }

    // --- Named lock tests ---

    #[test]
    fn test_named_lock_creates_named_file() {
        let tmp = TempDir::new().unwrap();
        let result = ContextLock::acquire(tmp.path(), "test_run", false, Some("planner")).unwrap();
        let named_file = tmp.path().join(".rings.lock.planner");
        let default_file = tmp.path().join(".rings.lock");
        assert!(named_file.exists(), ".rings.lock.planner should exist");
        assert!(!default_file.exists(), ".rings.lock should not exist");
        drop(result.lock);
        assert!(
            !named_file.exists(),
            "named lock file should be removed on drop"
        );
    }

    #[test]
    fn test_unnamed_lock_creates_default_file_regression() {
        let tmp = TempDir::new().unwrap();
        let result = ContextLock::acquire(tmp.path(), "test_run", false, None).unwrap();
        let default_file = tmp.path().join(".rings.lock");
        assert!(default_file.exists());
        drop(result.lock);
    }

    #[test]
    fn test_two_different_names_can_coexist() {
        let tmp = TempDir::new().unwrap();
        let r1 = ContextLock::acquire(tmp.path(), "run1", false, Some("planner")).unwrap();
        let r2 = ContextLock::acquire(tmp.path(), "run2", false, Some("builder")).unwrap();
        assert!(tmp.path().join(".rings.lock.planner").exists());
        assert!(tmp.path().join(".rings.lock.builder").exists());
        drop(r1.lock);
        drop(r2.lock);
    }

    #[test]
    fn test_same_name_conflicts() {
        let tmp = TempDir::new().unwrap();
        // Create a named lock with our own (live) PID
        let lock_file = tmp.path().join(".rings.lock.planner");
        let active_lock = LockFile {
            run_id: "run1".to_string(),
            pid: std::process::id(),
        };
        std::fs::write(&lock_file, serde_json::to_string(&active_lock).unwrap()).unwrap();

        let err = ContextLock::acquire(tmp.path(), "run2", false, Some("planner")).unwrap_err();
        match err {
            LockError::ActiveProcess { run_id, .. } => assert_eq!(run_id, "run1"),
            _ => panic!("expected ActiveProcess"),
        }
    }

    #[test]
    fn test_named_does_not_conflict_with_unnamed() {
        let tmp = TempDir::new().unwrap();
        // Hold the default unnamed lock
        let r1 = ContextLock::acquire(tmp.path(), "run1", false, None).unwrap();
        // Named lock should succeed despite the unnamed lock being held
        let r2 = ContextLock::acquire(tmp.path(), "run2", false, Some("planner")).unwrap();
        drop(r1.lock);
        drop(r2.lock);
    }

    #[test]
    fn test_unnamed_does_not_conflict_with_named() {
        let tmp = TempDir::new().unwrap();
        let r1 = ContextLock::acquire(tmp.path(), "run1", false, Some("planner")).unwrap();
        let r2 = ContextLock::acquire(tmp.path(), "run2", false, None).unwrap();
        drop(r1.lock);
        drop(r2.lock);
    }

    #[test]
    fn test_stale_named_lock_detected_and_removed() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock.planner");
        let stale_lock = LockFile {
            run_id: "old_run".to_string(),
            pid: 0,
        };
        std::fs::write(&lock_file, serde_json::to_string(&stale_lock).unwrap()).unwrap();

        let result = ContextLock::acquire(tmp.path(), "new_run", false, Some("planner")).unwrap();
        assert!(result.stale_removed.is_some());
        assert_eq!(result.stale_removed.as_ref().unwrap().run_id, "old_run");
        drop(result.lock);
    }

    #[test]
    fn test_force_lock_named() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock.planner");
        let old_lock = LockFile {
            run_id: "old_run".to_string(),
            pid: std::process::id(),
        };
        std::fs::write(&lock_file, serde_json::to_string(&old_lock).unwrap()).unwrap();

        let result = ContextLock::acquire(tmp.path(), "new_run", true, Some("planner")).unwrap();
        assert!(result.stale_removed.is_none());
        let contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
        drop(result.lock);
    }

    #[test]
    fn test_drop_removes_named_lock_file() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock.planner");
        let result = ContextLock::acquire(tmp.path(), "test_run", false, Some("planner")).unwrap();
        assert!(lock_file.exists());
        drop(result.lock);
        assert!(!lock_file.exists());
    }

    #[test]
    fn test_error_message_named_lock_includes_name() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock.planner");
        let active_lock = LockFile {
            run_id: "run1".to_string(),
            pid: std::process::id(),
        };
        std::fs::write(&lock_file, serde_json::to_string(&active_lock).unwrap()).unwrap();

        let err = ContextLock::acquire(tmp.path(), "run2", false, Some("planner")).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("holds lock \"planner\""),
            "error message should contain 'holds lock \"planner\"', got: {msg}"
        );
    }

    #[test]
    fn test_error_message_unnamed_lock_regression() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        let active_lock = LockFile {
            run_id: "run1".to_string(),
            pid: std::process::id(),
        };
        std::fs::write(&lock_file, serde_json::to_string(&active_lock).unwrap()).unwrap();

        let err = ContextLock::acquire(tmp.path(), "run2", false, None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("is already using"),
            "unnamed error message should contain 'is already using', got: {msg}"
        );
        assert!(
            !msg.contains("holds lock"),
            "unnamed error message should not contain 'holds lock', got: {msg}"
        );
    }

    #[test]
    fn test_lock_file_path_helper_unnamed() {
        let path = lock_file_path(Path::new("/tmp/foo"), None);
        assert_eq!(path, PathBuf::from("/tmp/foo/.rings.lock"));
    }

    #[test]
    fn test_lock_file_path_helper_named() {
        let path = lock_file_path(Path::new("/tmp/foo"), Some("planner"));
        assert_eq!(path, PathBuf::from("/tmp/foo/.rings.lock.planner"));
    }
}
