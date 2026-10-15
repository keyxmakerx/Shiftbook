//! File helpers: private folders and files, and atomic replacement.
//!
//! On Unix, folders are created `0700` and files `0600` so other accounts on
//! the machine can't read them. On Windows the per-user app data folder's
//! default permissions already limit access to the user.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

/// Create `dir` (and parents) if missing and make it private to the user.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Make an existing file readable and writable only by the user.
pub fn make_file_private(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Open a brand-new private file for writing; fails if it already exists.
fn create_new_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    options.open(path)
}

/// Flush a folder's entry list to disk so a rename survives a power cut (Unix only;
/// Windows has no equivalent for folders and its rename is already journaled).
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Replace `path` with `contents` atomically: write a temporary file in the same
/// folder, flush it, rename it over `path`, then flush the folder. A crash leaves
/// either the old file or the new one, never a half-written file.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no folder"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let mut suffix = [0u8; 8];
    getrandom::fill(&mut suffix).map_err(|_| io::Error::other("no random numbers"))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".tmp-{}", crate::hex::encode(&suffix)));
    let tmp = dir.join(tmp_name);

    let result = (|| {
        let mut file = create_new_private(&tmp)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        sync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
pub(crate) mod test_dir {
    use std::path::{Path, PathBuf};

    /// A unique folder under the system temp folder, removed when dropped.
    pub struct TestDir(PathBuf);

    impl TestDir {
        pub fn new(label: &str) -> Self {
            let mut id = [0u8; 8];
            getrandom::fill(&mut id).expect("random");
            let path = std::env::temp_dir().join(format!(
                "shiftbook-test-{label}-{}",
                crate::hex::encode(&id)
            ));
            std::fs::create_dir_all(&path).expect("create test dir");
            Self(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_dir::TestDir;
    use super::*;

    #[test]
    fn atomic_write_creates_and_replaces() {
        let dir = TestDir::new("atomic");
        let path = dir.path().join("file.json");
        write_atomic(&path, b"one").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"one");
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        // No temporary files are left behind.
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[cfg(unix)]
    #[test]
    fn private_permissions_on_unix() {
        let dir = TestDir::new("perms");
        let sub = dir.path().join("data");
        create_private_dir(&sub).unwrap();
        let mode = fs::metadata(&sub).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let path = sub.join("vault.json");
        write_atomic(&path, b"{}").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
