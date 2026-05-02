use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use std::{collections::HashMap, path::Path};

use crate::generate::Metadata;

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct MetadataCache {
    pub(crate) entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CacheEntry {
    pub(crate) mtime: SystemTime,
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

    pub(crate) fn prune(&mut self, markdown_dir: &Path) {
        self.entries
            .retain(|path, _| markdown_dir.join(path).exists());
    }
}
