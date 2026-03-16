#![cfg(unix)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockFile {
    pub run_id: String,
    pub pid: u32,
}

#[derive(Debug, Error)]
pub enum LockError {
    #[error("Error: Another rings run ({run_id}, PID={pid}) is already using context_dir.\nWait for it to finish or use --force-lock to override.")]
    ActiveProcess {
        run_id: String,
        pid: u32,
        context_dir: PathBuf,
    },
    #[error("context_dir does not exist: {}", path.display())]
    ContextDirMissing { path: PathBuf },
    #[error("failed to access lock file: {0}")]
    Io(#[from] std::io::Error),
}

impl From<serde_json::Error> for LockError {
    fn from(e: serde_json::Error) -> Self {
        LockError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            e.to_string(),
        ))
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
    ) -> Result<Self, LockError> {
        let context_dir = context_dir.as_ref();
        let run_id = run_id.as_ref();

        // Check context_dir exists
        if !context_dir.is_dir() {
            return Err(LockError::ContextDirMissing {
                path: context_dir.to_path_buf(),
            });
        }

        let lock_file_path = context_dir.join(".rings.lock");
        let lock_data = LockFile {
            run_id: run_id.to_string(),
            pid: std::process::id(),
        };
        let lock_json = serde_json::to_string(&lock_data).unwrap();

        if force {
            // Force overwrite
            let mut f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&lock_file_path)?;
            f.write_all(lock_json.as_bytes())?;
            return Ok(ContextLock {
                path: lock_file_path,
            });
        }

        // Attempt atomic write with O_CREAT|O_EXCL
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_file_path)
        {
            Ok(mut f) => {
                f.write_all(lock_json.as_bytes())?;
                Ok(ContextLock {
                    path: lock_file_path,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lock file exists. Check if it's stale.
                if let Ok(existing_lock) = Self::read_lock_file(&lock_file_path) {
                    // Check if the pid in the lock file is alive
                    if Self::is_process_alive(existing_lock.pid) {
                        return Err(LockError::ActiveProcess {
                            run_id: existing_lock.run_id,
                            pid: existing_lock.pid,
                            context_dir: context_dir.to_path_buf(),
                        });
                    }
                }
                // Lock is stale or unreadable. Try to remove and retry once.
                let _ = std::fs::remove_file(&lock_file_path);

                // Second attempt
                match OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock_file_path)
                {
                    Ok(mut f) => {
                        f.write_all(lock_json.as_bytes())?;
                        Ok(ContextLock {
                            path: lock_file_path,
                        })
                    }
                    Err(_) => {
                        // Second attempt failed, probably another process got it.
                        // Try to read it again to give a better error.
                        if let Ok(existing_lock) = Self::read_lock_file(&lock_file_path) {
                            Err(LockError::ActiveProcess {
                                run_id: existing_lock.run_id,
                                pid: existing_lock.pid,
                                context_dir: context_dir.to_path_buf(),
                            })
                        } else {
                            // Couldn't read, just report ActiveProcess with a placeholder
                            Err(LockError::ActiveProcess {
                                run_id: "unknown".to_string(),
                                pid: 0,
                                context_dir: context_dir.to_path_buf(),
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
        let lock = ContextLock::acquire(tmp.path(), "test_run", false).unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        assert!(lock_file.exists());
        drop(lock);
        assert!(!lock_file.exists());
    }

    #[test]
    fn test_lock_file_format() {
        let tmp = TempDir::new().unwrap();
        let _lock = ContextLock::acquire(tmp.path(), "test_run_123", false).unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        let contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.run_id, "test_run_123");
        assert!(parsed.pid > 0);
    }

    #[test]
    fn test_context_dir_missing() {
        let err = ContextLock::acquire("/nonexistent/path", "test_run", false).unwrap_err();
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
        let lock = ContextLock::acquire(tmp.path(), "new_run", false).unwrap();
        assert_eq!(lock.path, lock_file);

        let new_contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&new_contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
    }

    #[test]
    fn test_empty_lock_file_treated_as_stale() {
        let tmp = TempDir::new().unwrap();
        let lock_file = tmp.path().join(".rings.lock");
        std::fs::write(&lock_file, "").unwrap();

        // Should succeed and write a new lock
        let _lock = ContextLock::acquire(tmp.path(), "new_run", false).unwrap();
        let contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
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
        let _lock = ContextLock::acquire(tmp.path(), "new_run", true).unwrap();
        let new_contents = std::fs::read_to_string(&lock_file).unwrap();
        let parsed: LockFile = serde_json::from_str(&new_contents).unwrap();
        assert_eq!(parsed.run_id, "new_run");
    }
}
