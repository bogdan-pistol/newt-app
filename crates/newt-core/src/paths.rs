//! Where Newt's files live on disk.
//!
//! All paths derive from a single "home" directory:
//! - `$NEWT_HOME` if set (used by tests and for sandboxed runs)
//! - else `~/Library/Application Support/Newt` on macOS
//!
//! Only one place in the code computes these paths, so tests can override
//! the home root and every consumer sees the override.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    pub home: PathBuf,
}

impl Paths {
    /// Resolve paths from the environment. Honours `NEWT_HOME` if set.
    pub fn from_env() -> Result<Self> {
        if let Ok(p) = std::env::var("NEWT_HOME") {
            return Ok(Self { home: PathBuf::from(p) });
        }
        let home = std::env::var("HOME").context("HOME environment variable not set")?;
        Ok(Self {
            home: PathBuf::from(home).join("Library/Application Support/Newt"),
        })
    }

    /// Force a specific home (for tests).
    pub fn with_home(home: impl AsRef<Path>) -> Self {
        Self { home: home.as_ref().to_path_buf() }
    }

    pub fn prompts_dir(&self) -> PathBuf {
        self.home.join("prompts")
    }

    pub fn config_file(&self) -> PathBuf {
        self.home.join("config.toml")
    }

    /// Create the home directory and standard subdirectories if missing.
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.home)
            .with_context(|| format!("creating {}", self.home.display()))?;
        std::fs::create_dir_all(self.prompts_dir())
            .with_context(|| format!("creating {}", self.prompts_dir().display()))?;
        Ok(())
    }
}
