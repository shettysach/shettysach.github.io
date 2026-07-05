#![feature(if_let_guard)]

mod cache;
mod generate;
mod iter;
mod utils;
mod xml;

use anyhow::Result;
use std::{fs, path::Path};

fn main() -> Result<()> {
    let input_dir = Path::new("markdown");
    let static_dir = Path::new("static");
    let output_dir = Path::new("_site");
    let cache_path = Path::new("cache.json");

    if !output_dir.exists() {
        fs::create_dir(output_dir)?;
    }

    let mut cache = cache::MetadataCache::load(cache_path);
    generate::generate_site(input_dir, output_dir, &mut cache)?;
    cache.save(cache_path)?;

    utils::copy_directory(static_dir, output_dir)?;

    Ok(())
}
