use atomic_write_file::AtomicWriteFile;
use std::{
    fs::{self, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEW_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(bytes)?;
    file.commit()
}

struct PendingNewFile(PathBuf);

impl Drop for PendingNewFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(crate) fn atomic_write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| io::Error::from(ErrorKind::InvalidInput))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::from(ErrorKind::InvalidInput))?
        .to_string_lossy();
    let (temporary, mut file) = loop {
        let counter = NEW_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            counter
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (PendingNewFile(temporary), file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    rename_no_replace(&temporary.0, path)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub(crate) fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;

    fn verbatim_child(path: &Path) -> io::Result<Vec<u16>> {
        let file_name = path
            .file_name()
            .ok_or_else(|| io::Error::from(ErrorKind::InvalidInput))?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let path = fs::canonicalize(parent)?.join(file_name);
        Ok(path.as_os_str().encode_wide().chain(Some(0)).collect())
    }

    let source = verbatim_child(source)?;
    let destination = verbatim_child(destination)?;
    // SAFETY: Both vectors are live, NUL-terminated UTF-16 paths. With no
    // replace or copy flag, MoveFileExW is an exclusive same-volume rename.
    let result = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
pub(crate) fn rename_no_replace(_source: &Path, _destination: &Path) -> io::Result<()> {
    Err(io::Error::new(
        ErrorKind::Unsupported,
        "atomic no-replace rename is unsupported on this platform",
    ))
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos", windows)))]
mod tests {
    use super::rename_no_replace;
    use std::{fs, io::ErrorKind};

    #[test]
    fn no_replace_rename_moves_one_name_and_preserves_collisions() {
        let cleanup = std::env::temp_dir().join(format!(
            "typerlude-no-replace-{}-{}",
            std::process::id(),
            fastrand::u64(..)
        ));
        let root = (0..12).fold(cleanup.clone(), |path, index| {
            path.join(format!("extended-path-segment-{index:02}"))
        });
        fs::create_dir_all(&root).unwrap();

        let source = root.join("source");
        let destination = root.join("destination");
        fs::write(&source, b"source").unwrap();
        rename_no_replace(&source, &destination).unwrap();
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"source");

        let collision = root.join("collision");
        fs::write(&source, b"preserved source").unwrap();
        fs::write(&collision, b"preserved destination").unwrap();
        let error = rename_no_replace(&source, &collision).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&source).unwrap(), b"preserved source");
        assert_eq!(fs::read(&collision).unwrap(), b"preserved destination");

        fs::remove_dir_all(cleanup).unwrap();
    }
}
