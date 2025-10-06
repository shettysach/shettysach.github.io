use std::fs;
use std::io;
use std::path::Path;
use walkdir::WalkDir;

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
