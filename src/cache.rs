use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

use crate::generate::Metadata;

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct MetadataCache {
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CacheEntry {
    pub(crate) mtime: DateTime<Utc>,
    pub(crate) metadata: Metadata,
}

impl MetadataCache {
    pub(crate) fn load(path: &Path) -> Self {
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Ok(cache) = serde_json::from_str(&contents)
        {
            cache
        } else {
            Self::default()
        }
    }

    pub(crate) fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub(crate) fn get_entry(&self, rel_path: &str) -> Option<&CacheEntry> {
        self.entries.get(rel_path)
    }

    pub(crate) fn insert(&mut self, rel_path: &str, entry: CacheEntry) {
        self.entries.insert(rel_path.to_string(), entry);
    }

    pub(crate) fn prune(&mut self, markdown_dir: &Path) {
        self.entries
            .retain(|path, _| markdown_dir.join(path).exists());
    }
}
