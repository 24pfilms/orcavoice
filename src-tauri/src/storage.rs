use crate::error::AppError;
use std::fs;
use std::path::{Path, PathBuf};

/// Write a file so it is either fully old or fully new, never half-written.
///
/// `fs::write` truncates in place, so a crash or forced kill between the
/// truncate and the flush leaves a file whose length is correct but whose
/// contents are NUL bytes. That is exactly how `history.json` became 94,916
/// zero bytes, which then failed every later dictation. Writing to a temp file,
/// fsyncing it, and renaming over the target makes the swap atomic on NTFS and
/// POSIX alike.
pub fn write_atomic(path: &Path, contents: &str) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Config(format!("Path has no parent directory: {}", path.display())))?;
    fs::create_dir_all(parent)
        .map_err(|e| AppError::Config(format!("Cannot create directory {}: {e}", parent.display())))?;

    let temp_path = temp_path_for(path);
    {
        use std::io::Write;
        let mut file = fs::File::create(&temp_path).map_err(|e| {
            AppError::Config(format!("Cannot create temp file {}: {e}", temp_path.display()))
        })?;
        file.write_all(contents.as_bytes()).map_err(|e| {
            AppError::Config(format!("Cannot write temp file {}: {e}", temp_path.display()))
        })?;
        // Force the bytes to disk before the rename, otherwise the rename can
        // land first and still expose an empty file after a power loss.
        file.sync_all().map_err(|e| {
            AppError::Config(format!("Cannot flush temp file {}: {e}", temp_path.display()))
        })?;
    }

    fs::rename(&temp_path, path).map_err(|e| {
        let _ = fs::remove_file(&temp_path);
        AppError::Config(format!("Cannot replace {}: {e}", path.display()))
    })
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Move an unreadable file aside so the app can start clean instead of failing
/// forever on the same corrupt bytes. Returns the quarantine path when moved.
pub fn quarantine(path: &Path) -> Option<PathBuf> {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let mut name = path.file_name()?.to_os_string();
    name.push(format!(".corrupt-{stamp}"));
    let target = path.with_file_name(name);
    match fs::rename(path, &target) {
        Ok(()) => Some(target),
        Err(_) => {
            // Cannot preserve it; removing is still better than wedging the app.
            let _ = fs::remove_file(path);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orcavoice-storage-{tag}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_atomic_creates_and_replaces_contents() {
        let dir = temp_dir("write");
        let path = dir.join("data.json");
        write_atomic(&path, "[1]").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "[1]");
        write_atomic(&path, "[2]").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "[2]");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_leaves_no_temp_file_behind() {
        let dir = temp_dir("notemp");
        let path = dir.join("data.json");
        write_atomic(&path, "[]").unwrap();
        assert!(!dir.join("data.json.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn quarantine_moves_the_file_aside() {
        let dir = temp_dir("quarantine");
        let path = dir.join("history.json");
        fs::write(&path, vec![0_u8; 32]).unwrap();
        let moved = quarantine(&path).expect("file should be preserved");
        assert!(!path.exists());
        assert!(moved.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
