mod atom;
mod generate;
mod sitemap;
mod syntex;
mod toc;
mod types;
mod utils;

use anyhow::Result;
use std::{fs, path::Path};

fn main() -> Result<()> {
    let input_dir = Path::new("markdown");
    let others_dir = Path::new("others");
    let output_dir = Path::new("_site");

    if !output_dir.exists() {
        fs::create_dir(output_dir)?;
    }

    generate::ssgenerate(input_dir, output_dir)?;
    utils::copy_directory(others_dir, output_dir)?;

    Ok(())
}
