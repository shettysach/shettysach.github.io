use anyhow::Result;
use std::{fs, path::Path};
use walkdir::WalkDir;

pub(crate) fn html_escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            _ => result.push(c),
        }
    }
    result
}

pub(crate) fn slugify(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut last_dash = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            last_dash = false;
            result.push(c.to_ascii_lowercase());
        } else if !last_dash {
            last_dash = true;
            result.push('-');
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    result
}

pub(crate) fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    for entry in WalkDir::new(src).max_depth(1).into_iter().flatten() {
        let source = entry.path();
        let rel_path = source.strip_prefix(src)?;
        let target = dst.join(rel_path);

        if entry.file_type().is_file()
            && (!target.exists()
                || entry.metadata()?.modified()? > target.metadata()?.modified()?)
        {
            fs::copy(source, &target)?;
        }
    }

    Ok(())
}
