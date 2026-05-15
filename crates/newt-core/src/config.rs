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
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        paths.ensure_dirs()?;
        let path = paths.config_file();
        let text = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
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

    // ──────────────────────── typed settings ────────────────────────
    //
    // These accessors graduate specific keys from the flat string map to
    // typed reads with sensible defaults. The on-disk shape is unchanged
    // (still TOML key=value); the typing happens in Rust at read time.
    // PRD §5.7 will eventually grow this further (hotkey, launch-at-login,
    // telemetry opt-in) — same pattern.

    /// The provider used for every rewrite. Falls back to `mock` if unset
    /// so a fresh install still has *something* to run against.
    pub fn active_provider(&self) -> String {
        self.get("active_provider").unwrap_or("mock").to_string()
    }

    pub fn set_active_provider(&mut self, provider: &str) {
        self.set("active_provider", provider);
    }

    /// All models the user has added for `provider`. Empty list is
    /// allowed in storage (means "delete all"); on read we fall back to
    /// the bundled default so the UI never has to deal with an empty
    /// list. Reads the old singular `provider.<name>.model` key as a
    /// one-element fallback (auto-migration on first read).
    pub fn provider_models(&self, provider: &str) -> Vec<String> {
        let key = format!("provider.{provider}.models");
        if let Some(list) = self.get(&key) {
            let parsed: Vec<String> = list
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !parsed.is_empty() {
                return parsed;
            }
        }
        // Backwards-compat: pre-list schema stored a single value.
        let single_key = format!("provider.{provider}.model");
        if let Some(model) = self.get(&single_key) {
            return vec![model.to_string()];
        }
        // Default.
        match provider {
            "openai" => vec!["gpt-5.4-mini".to_string()],
            "anthropic" => vec!["claude-haiku-4-5-20251001".to_string()],
            _ => vec![],
        }
    }

    /// Replace the full list of models for `provider`. Empty/blank
    /// entries are filtered. Also clears the legacy singular key so
    /// old + new don't disagree after a write.
    pub fn set_provider_models(&mut self, provider: &str, models: &[String]) {
        let cleaned: Vec<&str> = models
            .iter()
            .map(|m| m.trim())
            .filter(|m| !m.is_empty())
            .collect();
        let key = format!("provider.{provider}.models");
        if cleaned.is_empty() {
            self.values.remove(&key);
        } else {
            self.set(key, cleaned.join(","));
        }
        // Drop the legacy singular key — list is the source of truth now.
        self.values.remove(&format!("provider.{provider}.model"));
    }

    /// The model currently selected for use by `provider`. Validates
    /// against the stored list — if the saved current doesn't match any
    /// entry (e.g. the user removed it), falls back to the first in the
    /// list. Always returns a non-empty string for known providers.
    pub fn provider_current_model(&self, provider: &str) -> String {
        let models = self.provider_models(provider);
        let key = format!("provider.{provider}.current_model");
        if let Some(current) = self.get(&key)
            && models.iter().any(|m| m == current)
        {
            return current.to_string();
        }
        // Fall back to first entry; empty string if no models at all
        // (only possible for unknown providers).
        models.into_iter().next().unwrap_or_default()
    }

    pub fn set_provider_current_model(&mut self, provider: &str, model: &str) {
        let key = format!("provider.{provider}.current_model");
        if model.trim().is_empty() {
            self.values.remove(&key);
        } else {
            self.set(key, model.trim());
        }
    }
}
