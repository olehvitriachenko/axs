use std::fs::{self, File, Metadata};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use axs::lexer::MAX_INPUT_BYTES;
use axs::{AxsError, ErrorKind, SourceSpan};
use tempfile::Builder;

use super::Failure;

pub(super) struct Loaded {
    pub source: String,
    path: PathBuf,
    metadata: Metadata,
}

pub(super) fn read(path: &Path, in_place: bool) -> Result<Loaded, Failure> {
    let target = if in_place {
        // Resolve parent symlinks once, but never follow the leaf symlink.
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent)
            .map_err(|error| Failure::io("resolving input directory", error))?;
        let name = path.file_name().ok_or_else(|| {
            Failure::io(
                "input path",
                io::Error::new(io::ErrorKind::InvalidInput, "Expected a file name"),
            )
        })?;
        parent.join(name)
    } else {
        path.to_owned()
    };
    let link = fs::symlink_metadata(&target)
        .map_err(|error| Failure::io(&format!("opening {path:?}"), error))?;
    if in_place && link.file_type().is_symlink() {
        return Err(Failure::io(
            "formatting input",
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "In-place formatting refuses symlinks",
            ),
        ));
    }
    let metadata =
        fs::metadata(&target).map_err(|error| Failure::io("reading input metadata", error))?;
    if !metadata.is_file() {
        return Err(Failure::io(
            "reading input",
            io::Error::new(io::ErrorKind::InvalidInput, "Expected a regular file"),
        ));
    }
    let file = File::open(&target).map_err(|error| Failure::io("opening input", error))?;
    let opened = file
        .metadata()
        .map_err(|error| Failure::io("reading open-file metadata", error))?;
    if !same_snapshot(&metadata, &opened) {
        return Err(changed());
    }
    let mut bytes = Vec::new();
    file.take((MAX_INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| Failure::io("reading input", error))?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(Failure::document(
            AxsError::new(
                ErrorKind::SyntaxError,
                "Maximum input size exceeded (at most 8 MiB)",
            )
            .with_span(SourceSpan::new(1, 1, 1)),
            "",
            path,
        ));
    }
    let source =
        String::from_utf8(bytes).map_err(|_| Failure::invalid("Input is not valid UTF-8"))?;
    Ok(Loaded {
        source,
        path: target,
        metadata: opened,
    })
}

pub(super) fn replace(original: &Loaded, bytes: &[u8]) -> Result<(), Failure> {
    if original.metadata.permissions().readonly() {
        return Err(Failure::io(
            "formatting input",
            io::Error::new(io::ErrorKind::PermissionDenied, "Input is read-only"),
        ));
    }
    let parent = original.path.parent().ok_or_else(|| {
        Failure::io(
            "input directory",
            io::Error::new(io::ErrorKind::InvalidInput, "Missing parent directory"),
        )
    })?;
    // Secure unique creation; NamedTempFile removes temporary files on failures.
    let mut temporary = Builder::new()
        .prefix(".axs-")
        .tempfile_in(parent)
        .map_err(|error| Failure::io("creating temporary file beside input", error))?;
    temporary
        .write_all(bytes)
        .map_err(|error| Failure::io("writing temporary file", error))?;
    temporary
        .as_file()
        .set_permissions(original.metadata.permissions())
        .map_err(|error| Failure::io("preserving file permissions", error))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| Failure::io("syncing temporary file", error))?;
    // Best-effort concurrent-edit detection, not atomic compare-and-swap against
    // adversarial writers in the containing directory.
    let current = read(&original.path, true)?;
    if !same_snapshot(&original.metadata, &current.metadata) || original.source != current.source {
        return Err(changed());
    }
    drop(current); // No source handle remains open during Windows replacement.
    temporary
        .persist(&original.path)
        .map_err(|error| Failure::io("replacing input file", error.error))?;
    Ok(())
}

fn changed() -> Failure {
    Failure::io(
        "formatting input",
        io::Error::other("Input changed while it was being read/formatted; refusing replacement"),
    )
}

fn same_snapshot(a: &Metadata, b: &Metadata) -> bool {
    if a.len() != b.len()
        || a.modified().ok() != b.modified().ok()
        || a.permissions().readonly() != b.permissions().readonly()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino() && a.mode() == b.mode()
    }
    #[cfg(not(unix))]
    {
        a.created().ok() == b.created().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_edit_is_preserved_and_temporary_file_is_cleaned() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.axs");
        fs::write(&path, "x=1\n").unwrap();
        let original = read(&path, true).unwrap_or_else(|error| panic!("{}", error.message));
        fs::write(&path, "x=20\n").unwrap();
        let result = replace(&original, b"x = 1\n");
        let Err(error) = result else {
            panic!("Concurrent change must fail")
        };
        assert_eq!(error.code, 3);
        assert_eq!(fs::read_to_string(path).unwrap(), "x=20\n");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_revalidation_preserves_target_and_cleans_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input.axs");
        fs::write(&path, "x=1\n").unwrap();
        let original = read(&path, true).unwrap_or_else(|error| panic!("{}", error.message));
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(replace(&original, b"x = 1\n").is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
