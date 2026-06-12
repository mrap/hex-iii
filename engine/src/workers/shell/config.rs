// Copyright Motia LLC and/or licensed to Motia LLC under one or more
// contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.
// This software is patent protected. We welcome discussions - reach out at team@iii.dev
// See LICENSE and PATENTS files for details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecConfig {
    pub watch: Option<Vec<String>>,
    pub exec: Vec<String>,
    /// Optional crash-respawn policy for the long-lived (last) command. With
    /// no `restart`, the daemon is spawned once and never respawned — the
    /// legacy behavior.
    #[serde(default)]
    pub restart: Option<RestartPolicy>,
    /// Optional liveness probe for the long-lived command. With no `health`,
    /// the daemon is never probed.
    #[serde(default)]
    pub health: Option<HealthCheck>,
}

/// Respawn the long-lived command when it exits, with exponential backoff.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RestartPolicy {
    /// Respawn when the daemon exits on its own (for any reason). When false,
    /// an exit is observed and logged but not acted on.
    pub on_crash: bool,
    /// Initial delay before the first respawn. Doubles on each consecutive
    /// crash up to `max_backoff_secs`.
    #[serde(default = "default_backoff_secs")]
    pub backoff_secs: u64,
    /// Ceiling for the exponential backoff.
    #[serde(default = "default_max_backoff_secs")]
    pub max_backoff_secs: u64,
}

/// Liveness probe. Exactly one of `url` (HTTP 2xx = healthy) or `command`
/// (exit 0 = healthy) should be set; `command` wins if both are present.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HealthCheck {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_interval_secs")]
    pub interval_secs: u64,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Consecutive probe failures before the daemon is killed and respawned.
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
}

fn default_backoff_secs() -> u64 {
    5
}
fn default_max_backoff_secs() -> u64 {
    60
}
fn default_interval_secs() -> u64 {
    30
}
fn default_timeout_secs() -> u64 {
    5
}
fn default_failure_threshold() -> u32 {
    3
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exec_config_roundtrip() {
        let json = json!({"exec": ["node", "index.js"], "watch": ["src"]});
        let config: ExecConfig = serde_json::from_value(json).unwrap();
        assert_eq!(config.exec, vec!["node", "index.js"]);
        assert_eq!(config.watch, Some(vec!["src".to_string()]));
        let back = serde_json::to_value(&config).unwrap();
        assert_eq!(back["exec"][0], "node");
    }

    #[test]
    fn exec_config_optional_watch() {
        let json = json!({"exec": ["python", "main.py"]});
        let config: ExecConfig = serde_json::from_value(json).unwrap();
        assert!(config.watch.is_none());
    }
}
