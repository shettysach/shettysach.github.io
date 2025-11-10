use anyhow::Result;
use std::{fs, path::Path};
use walkdir::WalkDir;
use xxhash_rust::xxh32::xxh32;

// styles_dir

pub(crate) fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src).max_depth(2).into_iter().flatten() {
        let source = entry.path();
        let rel_path = source.strip_prefix(src)?;
        let target = dst.join(rel_path);

        if entry.file_type().is_file()
            && (!target.exists()
                || entry.metadata()?.modified()? > target.metadata()?.modified()?)
        {
            fs::copy(source, &target)?;
        }

        // else if entry.file_type().is_dir() && !target.exists() {
        //     fs::create_dir_all(&target)?;
        // }
    }

    Ok(())
}

// Slugger

pub struct Slugger {
    counts: [u8; 64],
}

impl Slugger {
    pub(crate) fn new() -> Self {
        Slugger { counts: [0; 64] }
    }

    pub(crate) fn slug(&mut self, input: &str) -> String {
        let base = slugify(input);
        let key = xxh32(base.as_bytes(), 0).to_le() % 64;

        let c = &mut self.counts[key as usize];
        let slug = if *c != 0 {
            format!("{}_{}", base, *c)
        } else {
            base
        };

        *c += 1;
        slug
    }
}

fn slugify(s: &str) -> String {
    let mut last_dash = true;
    let iter = s.chars();

    iter.filter_map(|c| {
        if c.is_alphanumeric() {
            last_dash = false;
            Some(c.to_ascii_lowercase())
        } else if c.is_whitespace() && !last_dash {
            last_dash = true;
            Some('-')
        } else {
            None
        }
    })
    .collect()
}
