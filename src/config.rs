use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

/// Rings configuration loaded from config files.
///
/// Priority order (first found wins):
/// 1. `.rings-config.toml` in the current working directory
/// 2. `$XDG_CONFIG_HOME/rings/config.toml` (typically `~/.config/rings/config.toml`)
///
/// All fields are optional. Missing fields use built-in defaults.
#[derive(Debug, Default, Deserialize)]
pub struct RingsConfig {
    /// Default output directory for run data. Supports `~` expansion.
    pub default_output_dir: Option<String>,
    /// Enable colored output. Set to false to always disable color.
    pub color: Option<bool>,
    /// If true, suppresses the completion signal warning globally.
    pub skip_completion_check: Option<bool>,
    /// If true, low-confidence parse results halt execution (exit 2).
    pub strict_parsing: Option<bool>,
}

impl RingsConfig {
    /// Load configuration from the first config file found.
    ///
    /// Returns `Ok(RingsConfig::default())` if no config file exists.
    /// Prints an info message to stderr when a local `.rings-config.toml` is loaded.
    pub fn load() -> Result<Self> {
        let project_dir = std::env::current_dir().context("Cannot determine current directory")?;
        let xdg_config_home = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        Self::load_from(&project_dir, xdg_config_home.as_deref())
    }

    /// Load configuration given explicit paths instead of relying on process-global state.
    ///
    /// - `project_dir`: directory to search for `.rings-config.toml`
    /// - `xdg_config_home`: if `Some`, used as the XDG config base; if `None`, falls back to
    ///   `dirs::config_dir()`
    pub fn load_from(
        project_dir: &std::path::Path,
        xdg_config_home: Option<&std::path::Path>,
    ) -> Result<Self> {
        // 1. Check for project-level config in project_dir
        let local_path = project_dir.join(".rings-config.toml");
        if local_path.exists() {
            eprintln!("Loading local config from ./.rings-config.toml");
            let content =
                std::fs::read_to_string(&local_path).context("Cannot read .rings-config.toml")?;
            let config: RingsConfig =
                toml::from_str(&content).context("Invalid TOML in .rings-config.toml")?;
            return Ok(config);
        }

        // 2. Check for user-level config via provided xdg_config_home or dirs::config_dir()
        let user_config_path = if let Some(xdg) = xdg_config_home {
            Some(xdg.join("rings").join("config.toml"))
        } else {
            dirs::config_dir().map(|d| d.join("rings").join("config.toml"))
        };
        if let Some(path) = user_config_path {
            if path.exists() {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Cannot read {}", path.display()))?;
                let config: RingsConfig = toml::from_str(&content)
                    .with_context(|| format!("Invalid TOML in {}", path.display()))?;
                return Ok(config);
            }
        }

        // Neither config file found — use empty defaults
        Ok(RingsConfig::default())
    }

    /// Expand `~` in a path string to the home directory.
    pub fn expand_tilde(path: &str) -> String {
        if let Some(stripped) = path.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(stripped).to_string_lossy().to_string();
            }
        }
        path.to_string()
    }

    /// Returns the expanded `default_output_dir`, if configured.
    pub fn expanded_output_dir(&self) -> Option<String> {
        self.default_output_dir.as_deref().map(Self::expand_tilde)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_project_config() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join(".rings-config.toml");
        let mut f = std::fs::File::create(&config_path).unwrap();
        writeln!(f, r#"default_output_dir = "~/my-runs""#).unwrap();
        writeln!(f, "color = false").unwrap();

        let cfg = RingsConfig::load_from(dir.path(), None).unwrap();
        assert_eq!(cfg.default_output_dir.as_deref(), Some("~/my-runs"));
        assert_eq!(cfg.color, Some(false));
    }

    #[test]
    fn test_load_user_config_when_no_project_config() {
        let dir = TempDir::new().unwrap();
        // Create a fake XDG_CONFIG_HOME with rings/config.toml
        let xdg_dir = TempDir::new().unwrap();
        let rings_cfg_dir = xdg_dir.path().join("rings");
        std::fs::create_dir_all(&rings_cfg_dir).unwrap();
        let cfg_path = rings_cfg_dir.join("config.toml");
        let mut f = std::fs::File::create(&cfg_path).unwrap();
        writeln!(f, r#"default_output_dir = "~/.local/share/rings/runs""#).unwrap();

        let cfg = RingsConfig::load_from(dir.path(), Some(xdg_dir.path())).unwrap();
        assert_eq!(
            cfg.default_output_dir.as_deref(),
            Some("~/.local/share/rings/runs")
        );
    }

    #[test]
    fn test_project_config_takes_precedence_over_user_config() {
        let dir = TempDir::new().unwrap();
        // Project config
        let mut pf = std::fs::File::create(dir.path().join(".rings-config.toml")).unwrap();
        writeln!(pf, r#"default_output_dir = "/project/output""#).unwrap();

        // User config (in XDG dir)
        let xdg_dir = TempDir::new().unwrap();
        let rings_cfg_dir = xdg_dir.path().join("rings");
        std::fs::create_dir_all(&rings_cfg_dir).unwrap();
        let mut uf = std::fs::File::create(rings_cfg_dir.join("config.toml")).unwrap();
        writeln!(uf, r#"default_output_dir = "/user/output""#).unwrap();

        let cfg = RingsConfig::load_from(dir.path(), Some(xdg_dir.path())).unwrap();
        // Project config wins
        assert_eq!(cfg.default_output_dir.as_deref(), Some("/project/output"));
    }

    #[test]
    fn test_missing_both_config_files_returns_empty_defaults() {
        let dir = TempDir::new().unwrap();
        let xdg_dir = TempDir::new().unwrap(); // No config.toml inside

        let cfg = RingsConfig::load_from(dir.path(), Some(xdg_dir.path())).unwrap();
        assert!(cfg.default_output_dir.is_none());
        assert!(cfg.color.is_none());
    }

    #[test]
    fn test_invalid_toml_produces_error() {
        let dir = TempDir::new().unwrap();
        let mut f = std::fs::File::create(dir.path().join(".rings-config.toml")).unwrap();
        writeln!(f, "this is not valid toml ===").unwrap();

        let result = RingsConfig::load_from(dir.path(), None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("TOML")
                || msg.contains("toml")
                || msg.contains("parse")
                || msg.contains("expected")
        );
    }

    #[test]
    fn test_expand_tilde() {
        if let Some(home) = dirs::home_dir() {
            let expanded = RingsConfig::expand_tilde("~/foo/bar");
            assert!(expanded.starts_with(home.to_string_lossy().as_ref()));
            assert!(expanded.ends_with("foo/bar"));
        }
        // No tilde - unchanged
        assert_eq!(RingsConfig::expand_tilde("/abs/path"), "/abs/path");
        assert_eq!(RingsConfig::expand_tilde("relative"), "relative");
    }
}
