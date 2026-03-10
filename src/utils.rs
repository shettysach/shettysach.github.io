use anyhow::Result;
use std::{fs, path::Path};
use walkdir::WalkDir;

// styles_dir

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
