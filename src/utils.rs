use std::{fs, io, path::Path};
use walkdir::WalkDir;

use rustc_hash::{FxBuildHasher, FxHashMap};
use xxhash_rust::xxh32::xxh32;

pub struct Slugger {
    counts: FxHashMap<u32, u8>,
}

impl Slugger {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Slugger {
            counts: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
        }
    }

    fn hash_heading(&mut self, slug: &str) -> u32 {
        let v = xxh32(slug.as_bytes(), 0);
        v.to_le()
    }

    pub(crate) fn slug(&mut self, input: &str) -> String {
        let base = slugify(input);
        let key = self.hash_heading(&base);

        match self.counts.get_mut(&key) {
            Some(c) => {
                *c += 1;
                format!("{}_{}", base, *c)
            }
            None => {
                self.counts.insert(key, 0);
                base
            }
        }
    }
}

pub fn slugify(s: &str) -> String {
    let mut last_dash = true;
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                last_dash = false;
                Some(c.to_ascii_lowercase())
            } else if !last_dash {
                last_dash = true;
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn copy_directory(src: &Path, dst: &Path) -> io::Result<()> {
    for entry in WalkDir::new(src).max_depth(2).into_iter().flatten() {
        let path = entry.path();
        let rel_path = path.strip_prefix(src).unwrap();
        let target_path = dst.join(rel_path);

        if entry.file_type().is_dir() && !target_path.exists() {
            fs::create_dir_all(&target_path)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target_path)?;
        }
    }
    Ok(())
}
