use std::{fs, io::Write, path::Path};

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
    let bytes = serde_json::to_vec_pretty(review).context("serialize comments")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary comments file in {}", parent.display()))?;
    match fs::metadata(path) {
        Ok(metadata) => temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .with_context(|| format!("preserve permissions from {}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("read permissions from {}", path.display()));
        }
    }
    temporary
        .write_all(&bytes)
        .with_context(|| format!("write temporary comments file for {}", path.display()))?;
    temporary
        .write_all(b"\n")
        .with_context(|| format!("finish temporary comments file for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("sync temporary comments file for {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("replace comments file {}", path.display()))?;
    Ok(())
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
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
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

    #[cfg(unix)]
    #[test]
    fn save_preserves_existing_sidecar_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir().unwrap();
        let path = directory.path().join("review.json");
        std::fs::write(&path, "{}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let source = SourceBuffer::from_bytes("sample", b"hello\n").unwrap();
        save_review(&path, &ReviewDocument::empty(source.source_ref())).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
