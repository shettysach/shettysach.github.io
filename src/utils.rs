use anyhow::Result;
use pulldown_cmark::HeadingLevel;
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

// Headings

use std::num::NonZeroU8;

#[derive(Clone, Copy)]
pub(crate) struct Levels(NonZeroU8);

impl Levels {
    #[inline]
    pub(crate) fn new(min: u8, max: u8) -> Option<Self> {
        ((1..=6).contains(&min) && (min..=6).contains(&max))
            .then_some(min << 4 | max)
            .and_then(NonZeroU8::new)
            .map(Self)
    }

    #[inline]
    pub(crate) fn min_level(&self) -> u8 {
        self.0.get() >> 4
    }

    #[inline]
    pub(crate) fn max_level(&self) -> u8 {
        self.0.get() & 0x0F
    }

    #[inline]
    pub(crate) fn level_to_u8(level: HeadingLevel) -> u8 {
        match level {
            HeadingLevel::H1 => 1,
            HeadingLevel::H2 => 2,
            HeadingLevel::H3 => 3,
            HeadingLevel::H4 => 4,
            HeadingLevel::H5 => 5,
            HeadingLevel::H6 => 6,
        }
    }

    #[inline]
    pub(crate) fn level_enabled(&self, level: HeadingLevel) -> bool {
        let level_num = Self::level_to_u8(level);
        (self.min_level()..=self.max_level()).contains(&level_num)
    }
}

// Slugger

pub struct Slugger<const N: usize> {
    counts: [u8; N],
}

impl<const N: usize> Slugger<N> {
    pub(crate) fn new() -> Self {
        Slugger { counts: [0; N] }
    }

    pub(crate) fn slug(&mut self, input: &str) -> String {
        let base = slugify(input);
        let hash = xxh32(base.as_bytes(), 0).to_le() as usize;
        let key = hash % N;

        let c = &mut self.counts[key];
        let slug = if *c != 0 {
            format!("{}_{}", base, *c)
        } else {
            base
        };

        *c = c.wrapping_add(1);
        slug
    }
}

impl<const N: usize> Default for Slugger<N> {
    fn default() -> Self {
        Self::new()
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
