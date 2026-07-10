use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::domain::ReviewDocument;

/// Loads a review sidecar from JSON.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed.
pub fn load_review(path: &Path) -> Result<ReviewDocument> {
    let bytes = fs::read(path).with_context(|| format!("read comments from {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse comments from {}", path.display()))
}

/// Atomically saves a review sidecar next to its destination.
///
/// # Errors
///
/// Returns an error when serialization or any filesystem operation fails.
pub fn save_review(path: &Path, review: &ReviewDocument) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("create comments directory {}", parent.display()))?;
    let temporary = temporary_path(path);
    let bytes = serde_json::to_vec_pretty(review).context("serialize comments")?;

    let mut file = fs::File::create(&temporary)
        .with_context(|| format!("create temporary comments file {}", temporary.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("write temporary comments file {}", temporary.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("finish temporary comments file {}", temporary.display()))?;
    file.sync_all()
        .with_context(|| format!("sync temporary comments file {}", temporary.display()))?;
    fs::rename(&temporary, path).with_context(|| {
        format!(
            "replace comments file {} with {}",
            path.display(),
            temporary.display()
        )
    })?;
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".tmp-{}", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{domain::ReviewDocument, source::SourceBuffer};

    use super::*;

    #[test]
    fn review_round_trips_through_an_atomic_sidecar() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("review.json");
        let source = SourceBuffer::from_bytes("sample", b"hello\n").unwrap();
        let review = ReviewDocument::empty(source.source_ref());

        save_review(&path, &review).unwrap();
        assert_eq!(load_review(&path).unwrap(), review);
        assert!(!temporary_path(&path).exists());
    }

    #[test]
    fn save_creates_parent_directories_and_load_reports_invalid_json() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("nested/review.json");
        let source = SourceBuffer::from_bytes("sample", b"hello\n").unwrap();
        let review = ReviewDocument::empty(source.source_ref());
        save_review(&path, &review).unwrap();
        assert_eq!(load_review(&path).unwrap(), review);

        std::fs::write(&path, b"not json").unwrap();
        let error = load_review(&path).unwrap_err().to_string();
        assert!(error.contains("parse comments"));
    }
}
