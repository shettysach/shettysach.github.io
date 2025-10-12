mod atom;
mod generate;
mod syntex;
mod utils;

use anyhow::Result;
use std::{fs, path::Path};

fn main() -> Result<()> {
    let input_dir = Path::new("markdown");
    let styles_dir = Path::new("styles");
    let output_dir = Path::new("_site");

    if !output_dir.exists() {
        fs::create_dir(output_dir)?;
    }

    generate::index_page(input_dir, output_dir)?;
    utils::copy_directory(styles_dir, output_dir)?;
    // utils::generate_code_css(styles_dir)?;

    Ok(())
}
