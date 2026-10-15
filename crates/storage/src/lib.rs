//! Shiftbook storage: the vault (database key wrapped by the master password and
//! the recovery key), the SQLCipher database with its migrations, and the
//! repositories.
//!
//! - [`vault`]: create/unlock/change password/regenerate recovery key.
//! - [`recovery`]: the recovery key's display and forgiving parsing.
//! - [`db`]: open the encrypted database with the DEK and run migrations.
//! - [`repo`]: repository traits and the SQLite implementation.
//!
//! Secrets live only in `secrecy`/`zeroize` types and never implement `Debug`
//! or `Serialize`.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod db;
pub mod error;
mod fsutil;
pub mod hex;
pub mod recovery;
pub mod repo;
pub mod vault;

pub use error::StorageError;
pub use fsutil::create_private_dir;
pub use recovery::RecoveryKey;
pub use repo::SqliteStore;
pub use vault::{Dek, Vault, VaultDir, VaultError};

/// The secret string type used for passwords and typed recovery keys.
pub use secrecy::{ExposeSecret, SecretString};

/// File name of the local database inside the app data directory.
pub const DATABASE_FILE_NAME: &str = "shiftbook.db";

/// A tiny Tokio runtime for async unit tests (Tokio's `macros` feature isn't enabled).
#[cfg(test)]
pub(crate) mod test_rt {
    use std::future::Future;

    pub fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future)
    }
}

#[cfg(test)]
mod repo_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir::TestDir;
    use crate::repo::{SettingsRepository, UserRepository};
    use crate::test_rt::block_on;

    #[test]
    fn database_file_name_matches_spec() {
        assert_eq!(DATABASE_FILE_NAME, "shiftbook.db");
    }

    /// The whole flow: setup → database → password change → recovery unlock. The
    /// database must open with the DEK from every route and never without it.
    #[test]
    fn database_opens_through_every_unlock_route() {
        let dir = TestDir::new("flow");
        let vault_dir = VaultDir::new_for_tests(dir.path());
        let db_path = db::database_path(dir.path());
        let pw = SecretString::from("first password here");
        let new_pw = SecretString::from("second password here");

        let (vault, key) = vault_dir.create(&pw).unwrap();
        let shown = key.display();
        block_on(async {
            let store = SqliteStore::new(db::open_encrypted(&db_path, vault.dek()).await.unwrap());
            let user = store.initialize_local_owner("Me").await.unwrap();
            store.set_setting(user.id, "marker", "hello").await.unwrap();
            store.pool().close().await;
        });
        drop(vault);

        vault_dir.change_password(&pw, &new_pw).unwrap();
        assert!(vault_dir.unlock_with_password(&pw).is_err());

        for vault in [
            vault_dir.unlock_with_password(&new_pw).unwrap(),
            vault_dir.unlock_with_recovery_key(&shown).unwrap(),
        ] {
            block_on(async {
                let store =
                    SqliteStore::new(db::open_encrypted(&db_path, vault.dek()).await.unwrap());
                let user = store.local_user().await.unwrap().unwrap();
                assert_eq!(
                    store
                        .get_setting(user.id, "marker")
                        .await
                        .unwrap()
                        .as_deref(),
                    Some("hello")
                );
                store.pool().close().await;
            });
        }

        let bytes = std::fs::read(&db_path).unwrap();
        assert_ne!(&bytes[..16], b"SQLite format 3\0");
    }
}
