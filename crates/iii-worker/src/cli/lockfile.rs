// Copyright Motia LLC and/or licensed to Motia LLC under one or more
// contributor license agreements. Licensed under the Elastic License 2.0;
// you may not use this file except in compliance with the Elastic License 2.0.
// This software is patent protected. We welcome discussions - reach out at support@motia.dev
// See LICENSE and PATENTS files for details.

//! Read and write `iii.lock` for reproducible managed worker installs.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

const LOCKFILE_VERSION: u8 = 1;
const LOCKFILE_NAME: &str = "iii.lock";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerLockfile {
    pub version: u8,
    pub workers: BTreeMap<String, LockedWorker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedWorker {
    pub version: String,
    #[serde(rename = "type")]
    pub worker_type: LockedWorkerType,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    pub source: LockedSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockedWorkerType {
    Binary,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LockedSource {
    Binary {
        target: String,
        url: String,
        sha256: String,
    },
    Image {
        image: String,
    },
}

impl Default for WorkerLockfile {
    fn default() -> Self {
        Self {
            version: LOCKFILE_VERSION,
            workers: BTreeMap::new(),
        }
    }
}

impl WorkerLockfile {
    pub fn from_yaml(input: &str) -> Result<Self, String> {
        serde_yaml::from_str(input).map_err(|e| format!("failed to parse {LOCKFILE_NAME}: {e}"))
    }

    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self)
            .map(|yaml| yaml.strip_prefix("---\n").unwrap_or(&yaml).to_string())
            .map_err(|e| format!("failed to serialize {LOCKFILE_NAME}: {e}"))
    }

    pub fn read_from(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::from_yaml(&content)
    }

    pub fn write_to(&self, path: &Path) -> Result<(), String> {
        let yaml = self.to_yaml()?;
        std::fs::write(path, yaml).map_err(|e| format!("failed to write {}: {e}", path.display()))
    }

    pub fn verify_config_workers(&self, worker_names: &[String]) -> Result<(), String> {
        let missing: Vec<&String> = worker_names
            .iter()
            .filter(|name| !self.workers.contains_key(*name))
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "{LOCKFILE_NAME} is missing worker(s): {}",
                missing
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

pub fn lockfile_path() -> &'static Path {
    Path::new(LOCKFILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn lockfile_round_trips_with_sorted_workers() {
        let mut lock = WorkerLockfile::default();
        lock.workers.insert(
            "z-worker".to_string(),
            LockedWorker {
                version: "1.0.0".to_string(),
                worker_type: LockedWorkerType::Binary,
                dependencies: BTreeMap::new(),
                source: LockedSource::Binary {
                    target: "aarch64-apple-darwin".to_string(),
                    url: "https://example.com/z.tar.gz".to_string(),
                    sha256: "z".repeat(64),
                },
            },
        );
        lock.workers.insert(
            "a-worker".to_string(),
            LockedWorker {
                version: "1.0.0".to_string(),
                worker_type: LockedWorkerType::Image,
                dependencies: BTreeMap::from([("z-worker".to_string(), "^1.0.0".to_string())]),
                source: LockedSource::Image {
                    image: "ghcr.io/iii-hq/a-worker@sha256:abc".to_string(),
                },
            },
        );

        let yaml = lock.to_yaml().unwrap();

        assert!(yaml.find("a-worker:").unwrap() < yaml.find("z-worker:").unwrap());
        let parsed = WorkerLockfile::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.workers["a-worker"].version, "1.0.0");
        assert_eq!(parsed.workers["z-worker"].version, "1.0.0");
    }

    #[test]
    fn stale_lock_detects_config_worker_missing_from_lock() {
        let lock = WorkerLockfile::default();

        let result = lock.verify_config_workers(&["hello-worker".to_string()]);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("hello-worker"));
    }
}
