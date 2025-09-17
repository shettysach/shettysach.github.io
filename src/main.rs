mod generate;
mod syntex;
mod utils;

use anyhow::Result;
use std::{fs, path::Path};
use syntect::{
    highlighting::ThemeSet,
    html::{ClassStyle, css_for_theme_with_class_style},
};

fn main() -> Result<()> {
    let markdown_dir = Path::new("markdown");
    let styles_dir = Path::new("styles");
    let html_dir = Path::new("_site");

    generate::index_page(markdown_dir, html_dir)?;
    utils::copy_directory(styles_dir, html_dir)?;

    let theme = &ThemeSet::load_from_folder("./metagen")?.themes["Enki-Tokyo-Night"];
    let code_css = css_for_theme_with_class_style(theme, ClassStyle::Spaced)?;
    fs::write(html_dir.join("css").join("code.css"), code_css)?;

    Ok(())
}
