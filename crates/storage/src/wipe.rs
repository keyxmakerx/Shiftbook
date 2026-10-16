//! Delete everything Shiftbook keeps in its data folder, for a user who wants to
//! start again (for example after losing both the password and the recovery key).
//!
//! The database goes first and the vault last, so a failure part-way leaves the
//! vault in place: the app still shows the unlock screen and the user can retry.
//! Once `vault.json` is gone the database key is gone with it, so any copy of the
//! database left on disk can't be opened.

use std::fs;
use std::io;
use std::path::Path;

use crate::vault::{VAULT_FILE_NAME, VAULT_PREV_FILE_NAME};
use crate::DATABASE_FILE_NAME;

/// SQLite's side files next to the database.
const DATABASE_SIDE_SUFFIXES: &[&str] = &["-journal", "-wal", "-shm"];

/// Remove a file; a file that is already gone is fine.
fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// True for a temporary file left by an interrupted atomic write of `name`
/// (`.<name>.tmp-<hex>`).
fn is_leftover_temp(file_name: &str, name: &str) -> bool {
    file_name
        .strip_prefix('.')
        .and_then(|rest| rest.strip_prefix(name))
        .is_some_and(|rest| rest.starts_with(".tmp-"))
}

/// Delete the database, its side files, the vault and its safety copy from `dir`.
/// The folder itself stays. Safe to call when some or all files are missing.
/// The database must be closed first.
pub fn delete_local_data(dir: &Path) -> io::Result<()> {
    remove_if_present(&dir.join(DATABASE_FILE_NAME))?;
    for suffix in DATABASE_SIDE_SUFFIXES {
        remove_if_present(&dir.join(format!("{DATABASE_FILE_NAME}{suffix}")))?;
    }
    match fs::read_dir(dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry?;
                let name = entry.file_name();
                let Some(name) = name.to_str() else { continue };
                if is_leftover_temp(name, VAULT_FILE_NAME)
                    || is_leftover_temp(name, VAULT_PREV_FILE_NAME)
                {
                    remove_if_present(&entry.path())?;
                }
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    }
    remove_if_present(&dir.join(VAULT_PREV_FILE_NAME))?;
    remove_if_present(&dir.join(VAULT_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{database_path, open_encrypted};
    use crate::fsutil::test_dir::TestDir;
    use crate::test_rt::block_on;
    use crate::{SecretString, VaultDir};

    #[test]
    fn spots_leftover_temp_files() {
        assert!(is_leftover_temp(".vault.json.tmp-0a1b", VAULT_FILE_NAME));
        assert!(!is_leftover_temp("vault.json", VAULT_FILE_NAME));
        assert!(!is_leftover_temp(".vault.json", VAULT_FILE_NAME));
        assert!(!is_leftover_temp(".other.json.tmp-0a1b", VAULT_FILE_NAME));
    }

    #[test]
    fn deletes_everything_and_keeps_other_files() {
        let dir = TestDir::new("wipe");
        let vault_dir = VaultDir::new_for_tests(dir.path());
        let (vault, _key) = vault_dir
            .create(&SecretString::from("a long enough password"))
            .unwrap();
        block_on(async {
            let pool = open_encrypted(&database_path(dir.path()), vault.dek())
                .await
                .unwrap();
            pool.close().await;
        });
        fs::write(dir.path().join(".vault.json.tmp-00ff"), b"x").unwrap();
        fs::write(dir.path().join("shiftbook.db-journal"), b"x").unwrap();
        fs::write(dir.path().join("portable-notes.txt"), b"keep").unwrap();
        assert!(vault_dir.exists());

        delete_local_data(dir.path()).unwrap();

        assert!(!vault_dir.exists());
        let mut left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, vec!["portable-notes.txt".to_string()]);

        // Nothing there (or no folder at all) is fine too.
        delete_local_data(dir.path()).unwrap();
        delete_local_data(&dir.path().join("missing")).unwrap();
    }
}
