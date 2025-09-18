mod generate;
mod syntex;
mod utils;

use anyhow::Result;
use std::{env, fs, path::Path};
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, css_for_theme_with_class_style},
};

fn main() -> Result<()> {
    let input_dir = Path::new("markdown");
    let styles_dir = Path::new("styles");
    let output_dir = Path::new("_site");

    generate::index_page(input_dir, output_dir)?;
    utils::copy_directory(styles_dir, output_dir)?;

    let theme = &ThemeSet::load_from_folder(env::current_dir()?)?.themes["Enki-Tokyo-Night"];
    let code_css = css_for_theme_with_class_style(theme, ClassStyle::Spaced)?;
    fs::write(output_dir.join("css").join("code.css"), code_css)?;

    Ok(())
}
