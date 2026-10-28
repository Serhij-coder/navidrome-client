use crate::{AtomicStore, Error, MAX_BYTES};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

/// Blocking adapter for an explicitly chosen development root. Call on a bounded
/// filesystem worker, never an async executor/UI thread. Does not search profiles.
pub struct NativeStore {
    directory: PathBuf,
    filename: &'static str,
}
impl NativeStore {
    pub fn open(development_root: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_file(development_root.as_ref(), "profiles-v1.json")
    }
    /// Separate reviewed-import workspace; never opens the original profile.
    pub fn open_imports(development_root: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_file(development_root.as_ref(), "imports-v1.json")
    }
    fn open_file(root: &Path, filename: &'static str) -> Result<Self, Error> {
        if !root.is_absolute() || root.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(Error::UnsafePath);
        }
        check_path(root)?;
        fs::create_dir_all(root).map_err(|_| Error::Io)?;
        let directory = root.join("feishin-rust-dev-p02");
        check_path(&directory)?;
        fs::create_dir_all(&directory).map_err(|_| Error::Io)?;
        Ok(Self {
            directory,
            filename,
        })
    }
    pub fn path(&self) -> PathBuf {
        self.directory.join(self.filename)
    }
}
fn check_path(path: &Path) -> Result<(), Error> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        // A Windows drive/device prefix is not a complete filesystem path.
        // Check it only once RootDir has made it a rooted path, then check
        // every subsequent component as usual (including symlink rejection).
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(Error::UnsafePath),
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err(Error::Io),
        }
    }
    Ok(())
}
impl AtomicStore for NativeStore {
    fn read(&mut self) -> Result<Option<Vec<u8>>, Error> {
        check_path(&self.path())?;
        let file = match fs::File::open(self.path()) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Io),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Io)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        Ok(Some(bytes))
    }
    fn replace(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::TooLarge);
        }
        check_path(&self.path())?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|_| Error::Io)?;
        temporary.write_all(bytes).map_err(|_| Error::Io)?;
        temporary.as_file().sync_all().map_err(|_| Error::Io)?;
        temporary.persist(self.path()).map_err(|_| Error::Io)?;
        #[cfg(unix)]
        fs::File::open(&self.directory)
            .and_then(|dir| dir.sync_all())
            .map_err(|_| Error::Io)?;
        Ok(())
    }
}
