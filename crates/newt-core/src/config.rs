//! Config: a flat string-keyed store persisted as TOML.
//!
//! Phase 0 exit criterion is just round-tripping arbitrary values, so the
//! schema is deliberately untyped. As later phases introduce real settings
//! (default provider, hotkey, telemetry opt-in), specific keys will graduate
//! to typed accessors layered on top of this same file format.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::paths::Paths;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten, default)]
    values: BTreeMap<String, String>,
}

impl Config {
    /// Load the config file. Missing file returns an empty config.
    pub fn load(paths: &Paths) -> Result<Self> {
        let path = paths.config_file();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        paths.ensure_dirs()?;
        let path = paths.config_file();
        let text = toml::to_string_pretty(self)
            .context("serializing config")?;
        std::fs::write(&path, text)
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn unset(&mut self, key: &str) -> bool {
        self.values.remove(key).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
