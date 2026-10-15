//! The vault session behind the vault commands, kept free of Tauri so it can be
//! unit-tested. One lock guards everything: while an unlock (or its back-off
//! delay) runs, other vault commands answer `busy` instead of queueing.
//!
//! Secrets arrive as `String` from IPC; they are moved into wiped buffers at once
//! and never logged, returned or stored. The only secret that goes back to the
//! WebView is the recovery key, once, right after setup.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use shiftbook_core::ids::OwnerId;
use shiftbook_storage::db::{database_path, open_encrypted};
use shiftbook_storage::repo::SettingsRepository;
use shiftbook_storage::{
    create_private_dir, ExposeSecret, SecretString, SqliteStore, StorageError, Vault, VaultDir,
    VaultError,
};
use tokio::sync::Mutex;
use zeroize::Zeroizing;

use crate::backoff::Backoff;

/// Name given to the single local user at setup.
const LOCAL_USER_NAME: &str = "Me";
/// Settings key recording when the user confirmed saving the recovery key.
pub const RECOVERY_SAVED_KEY: &str = "security.recovery_key_saved_at";

/// What the unlock screen should show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VaultStatus {
    /// First run: no vault yet.
    NeedsSetup,
    /// Set up, waiting for the password or recovery key.
    Locked,
    /// Open.
    Unlocked,
}

/// Errors as they cross IPC: short generic codes, never details.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandError {
    /// The input broke a rule (empty name, bad colour, too long...).
    InvalidInput,
    /// The record doesn't exist (or belongs to someone else).
    NotFound,
    /// The name is already used.
    Duplicate,
    /// The tray's roll-up entry gets its time and tickets from the tray.
    TrayManaged,
    /// Setting a new password without the old one is only allowed straight
    /// after a recovery-key unlock.
    NotAllowed,
    /// Wrong password or recovery key, or the key file was damaged.
    UnlockFailed,
    /// The new password breaks the rules (too short or too long).
    WeakPassword,
    /// Another vault command is still running.
    Busy,
    /// Setup was already done.
    AlreadySetUp,
    /// Unlock before setup.
    NotSetUp,
    /// The command needs the app to be unlocked.
    Locked,
    /// Anything else (disk, database, randomness). Details stay in the app.
    StorageError,
}

impl From<VaultError> for CommandError {
    fn from(e: VaultError) -> Self {
        match e {
            VaultError::UnlockFailed => CommandError::UnlockFailed,
            VaultError::WeakPassword(_) => CommandError::WeakPassword,
            VaultError::AlreadySetUp => CommandError::AlreadySetUp,
            VaultError::NotSetUp => CommandError::NotSetUp,
            VaultError::Io(_) | VaultError::Random | VaultError::Kdf => CommandError::StorageError,
        }
    }
}

impl From<StorageError> for CommandError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::Invalid(_) | StorageError::Protected(_) => CommandError::InvalidInput,
            StorageError::Duplicate(_) => CommandError::Duplicate,
            StorageError::NotFound => CommandError::NotFound,
            StorageError::TrayManaged => CommandError::TrayManaged,
            _ => CommandError::StorageError,
        }
    }
}

/// Which secret `unlock` was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// The master password.
    Password,
    /// The recovery key.
    RecoveryKey,
}

impl SecretKind {
    /// Parse the `kind` argument from the frontend.
    pub fn parse(kind: &str) -> Option<Self> {
        match kind {
            "password" => Some(SecretKind::Password),
            "recovery_key" => Some(SecretKind::RecoveryKey),
            _ => None,
        }
    }
}

/// An open session: the database and the key that opened it. Dropping it wipes
/// the key (`Vault` zeroizes on drop).
struct Session {
    store: SqliteStore,
    owner: OwnerId,
    vault: Arc<Vault>,
    /// Unlocked with the recovery key and no new password set yet.
    needs_new_password: bool,
}

#[derive(Default)]
struct Inner {
    session: Option<Session>,
    backoff: Backoff,
}

/// Turn a secret that arrived over IPC into a wiped-on-drop secret, wiping the
/// original buffer too.
pub fn take_secret(text: String) -> SecretString {
    let text = Zeroizing::new(text);
    SecretString::from(text.as_str())
}

/// Run blocking work (Argon2, file writes) off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| CommandError::StorageError)
}

/// The vault and database for one data folder.
pub struct VaultSession {
    data_dir: Option<PathBuf>,
    inner: Mutex<Inner>,
}

impl VaultSession {
    /// `data_dir` is `None` when the app data folder couldn't be found or
    /// created; every command then reports `storage_error`.
    pub fn new(data_dir: Option<PathBuf>) -> Self {
        Self {
            data_dir,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn vault_dir(&self) -> Result<VaultDir, CommandError> {
        self.data_dir
            .as_ref()
            .map(VaultDir::new)
            .ok_or(CommandError::StorageError)
    }

    fn data_dir(&self) -> Result<PathBuf, CommandError> {
        self.data_dir.clone().ok_or(CommandError::StorageError)
    }

    /// needs_setup, locked or unlocked.
    pub async fn status(&self) -> Result<VaultStatus, CommandError> {
        let vault_dir = self.vault_dir()?;
        let inner = self.inner.lock().await;
        Ok(if inner.session.is_some() {
            VaultStatus::Unlocked
        } else if vault_dir.exists() {
            VaultStatus::Locked
        } else {
            VaultStatus::NeedsSetup
        })
    }

    async fn open(&self, vault: Vault) -> Result<Session, CommandError> {
        let path = database_path(&self.data_dir()?);
        let pool = open_encrypted(&path, vault.dek()).await?;
        let store = SqliteStore::new(pool);
        let user = store.initialize_local_owner(LOCAL_USER_NAME).await?;
        Ok(Session {
            store,
            owner: user.id,
            vault: Arc::new(vault),
            needs_new_password: false,
        })
    }

    /// The open database and its owner, for data commands. Waits for a running
    /// vault command instead of answering `busy`.
    pub async fn data(&self) -> Result<(SqliteStore, OwnerId), CommandError> {
        let inner = self.inner.lock().await;
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        Ok((session.store.clone(), session.owner))
    }

    /// First-run setup. Returns the recovery key text, to be shown once.
    pub async fn setup(&self, password: SecretString) -> Result<SecretString, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let vault_dir = self.vault_dir()?;
        create_private_dir(vault_dir.dir()).map_err(|_| CommandError::StorageError)?;
        if vault_dir.exists() {
            return Err(CommandError::AlreadySetUp);
        }
        let (vault, key) = blocking(move || vault_dir.create(&password)).await??;
        let shown = key.display();
        drop(key);
        inner.session = Some(self.open(vault).await?);
        inner.backoff.record_success();
        Ok(shown)
    }

    /// Record that the user ticked "I saved my recovery key".
    pub async fn confirm_recovery_saved(&self) -> Result<(), CommandError> {
        let inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        let now = shiftbook_core::dates::format_timestamp(time::OffsetDateTime::now_utc());
        session
            .store
            .set_setting(session.owner, RECOVERY_SAVED_KEY, &now)
            .await?;
        Ok(())
    }

    /// Unlock with the password or the recovery key. After five failures each
    /// further failure waits longer before answering (1 s doubling to 60 s).
    /// Returns true when the user must now choose a new password (recovery key).
    pub async fn unlock(
        &self,
        secret: SecretString,
        kind: SecretKind,
    ) -> Result<bool, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        if let Some(session) = inner.session.as_ref() {
            return Ok(session.needs_new_password);
        }
        let vault_dir = self.vault_dir()?;
        let result = blocking(move || match kind {
            SecretKind::Password => vault_dir.unlock_with_password(&secret),
            SecretKind::RecoveryKey => vault_dir.unlock_with_recovery_key(&secret),
        })
        .await?;
        match result {
            Ok(vault) => {
                inner.backoff.record_success();
                let mut session = self.open(vault).await?;
                session.needs_new_password = kind == SecretKind::RecoveryKey;
                let needs = session.needs_new_password;
                inner.session = Some(session);
                Ok(needs)
            }
            Err(VaultError::UnlockFailed) => {
                let delay = inner.backoff.record_failure();
                Self::wait(delay).await;
                Err(CommandError::UnlockFailed)
            }
            Err(other) => Err(other.into()),
        }
    }

    async fn wait(delay: Duration) {
        if !delay.is_zero() {
            let _ = blocking(move || std::thread::sleep(delay)).await;
        }
    }

    /// Close the database and wipe the key. Safe to call when already locked.
    pub async fn lock(&self) -> Result<(), CommandError> {
        let mut inner = self.inner.lock().await;
        if let Some(session) = inner.session.take() {
            session.store.pool().close().await;
            drop(session);
        }
        Ok(())
    }

    /// Change the master password (needs the current one and an unlocked app).
    pub async fn change_password(
        &self,
        current: SecretString,
        new: SecretString,
    ) -> Result<(), CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        if inner.session.is_none() {
            return Err(CommandError::Locked);
        }
        let vault_dir = self.vault_dir()?;
        match blocking(move || vault_dir.change_password(&current, &new)).await? {
            Ok(()) => {
                inner.backoff.record_success();
                Ok(())
            }
            Err(VaultError::UnlockFailed) => {
                let delay = inner.backoff.record_failure();
                Self::wait(delay).await;
                Err(CommandError::UnlockFailed)
            }
            Err(other) => Err(other.into()),
        }
    }
}

impl VaultSession {
    /// Set a new password without the old one. Only straight after a
    /// recovery-key unlock (the owner decided to ask for one at once).
    pub async fn set_new_password(&self, new: SecretString) -> Result<(), CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let session = inner.session.as_mut().ok_or(CommandError::Locked)?;
        if !session.needs_new_password {
            return Err(CommandError::NotAllowed);
        }
        let vault_dir = self.vault_dir()?;
        let vault = Arc::clone(&session.vault);
        blocking(move || vault_dir.reset_password(&vault, &new)).await??;
        session.needs_new_password = false;
        Ok(())
    }

    /// Make a new recovery key (needs the password); the old key stops working.
    /// Returns the new key text, shown once.
    pub async fn regenerate_recovery_key(
        &self,
        password: SecretString,
    ) -> Result<SecretString, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        if inner.session.is_none() {
            return Err(CommandError::Locked);
        }
        let vault_dir = self.vault_dir()?;
        match blocking(move || vault_dir.regenerate_recovery_key(&password)).await? {
            Ok(key) => {
                inner.backoff.record_success();
                Ok(key.display())
            }
            Err(VaultError::UnlockFailed) => {
                let delay = inner.backoff.record_failure();
                Self::wait(delay).await;
                Err(CommandError::UnlockFailed)
            }
            Err(other) => Err(other.into()),
        }
    }
}

/// Expose the recovery key text for its one trip to the WebView.
pub fn reveal_once(key: SecretString) -> String {
    key.expose_secret().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let mut id = [0u8; 8];
            unique_bytes(&mut id);
            let path = std::env::temp_dir().join(format!(
                "shiftbook-desktop-{label}-{}",
                shiftbook_storage::hex::encode(&id)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn unique_bytes(buf: &mut [u8]) {
        // Unique enough for a temp folder name; no extra crate needed.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mixed = nanos ^ (u128::from(std::process::id()) << 64);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = (mixed >> (i * 8)) as u8;
        }
    }

    fn block_on<F: std::future::Future>(f: F) -> F::Output {
        tauri::async_runtime::block_on(f)
    }

    fn s(text: &str) -> SecretString {
        take_secret(text.to_string())
    }

    #[test]
    fn errors_cross_ipc_as_plain_codes() {
        assert_eq!(
            serde_json::to_value(CommandError::UnlockFailed).unwrap(),
            "unlock_failed"
        );
        assert_eq!(
            serde_json::to_value(CommandError::WeakPassword).unwrap(),
            "weak_password"
        );
        assert_eq!(serde_json::to_value(CommandError::Busy).unwrap(), "busy");
        assert_eq!(
            serde_json::to_value(VaultStatus::NeedsSetup).unwrap(),
            "needs_setup"
        );
        assert_eq!(SecretKind::parse("password"), Some(SecretKind::Password));
        assert_eq!(
            SecretKind::parse("recovery_key"),
            Some(SecretKind::RecoveryKey)
        );
        assert_eq!(SecretKind::parse("Password"), None);
    }

    #[test]
    fn no_data_folder_means_storage_error() {
        let session = VaultSession::new(None);
        assert_eq!(block_on(session.status()), Err(CommandError::StorageError));
    }

    /// The full flow with real (calibrated) Argon2 parameters.
    #[test]
    fn setup_lock_unlock_change_password() {
        let dir = TempDir::new("flow");
        let session = VaultSession::new(Some(dir.0.clone()));
        block_on(async {
            assert_eq!(session.status().await, Ok(VaultStatus::NeedsSetup));
            assert_eq!(
                session
                    .unlock(s("whatever password"), SecretKind::Password)
                    .await,
                Err(CommandError::NotSetUp)
            );
            assert_eq!(
                session.setup(s("too short")).await.err(),
                Some(CommandError::WeakPassword)
            );
            let key = reveal_once(session.setup(s("first password 1")).await.unwrap());
            assert_eq!(key.len(), 52 + 12);
            assert_eq!(session.status().await, Ok(VaultStatus::Unlocked));
            session.confirm_recovery_saved().await.unwrap();
            assert_eq!(
                session.setup(s("first password 1")).await.err(),
                Some(CommandError::AlreadySetUp)
            );

            session.lock().await.unwrap();
            assert_eq!(session.status().await, Ok(VaultStatus::Locked));
            assert_eq!(
                session.confirm_recovery_saved().await,
                Err(CommandError::Locked)
            );
            assert_eq!(
                session
                    .change_password(s("first password 1"), s("second password 2"))
                    .await,
                Err(CommandError::Locked)
            );
            assert_eq!(
                session
                    .unlock(s("wrong password!!"), SecretKind::Password)
                    .await,
                Err(CommandError::UnlockFailed)
            );
            session
                .unlock(s("first password 1"), SecretKind::Password)
                .await
                .unwrap();
            assert_eq!(session.status().await, Ok(VaultStatus::Unlocked));

            assert_eq!(
                session
                    .change_password(s("not current pw"), s("second password 2"))
                    .await,
                Err(CommandError::UnlockFailed)
            );
            assert_eq!(
                session
                    .change_password(s("first password 1"), s("short"))
                    .await,
                Err(CommandError::WeakPassword)
            );
            session
                .change_password(s("first password 1"), s("second password 2"))
                .await
                .unwrap();
            session.lock().await.unwrap();
            assert_eq!(
                session
                    .unlock(s("first password 1"), SecretKind::Password)
                    .await,
                Err(CommandError::UnlockFailed)
            );
            assert_eq!(
                session
                    .unlock(s(&key.to_lowercase()), SecretKind::RecoveryKey)
                    .await,
                Ok(true),
                "a recovery-key unlock asks for a new password"
            );
            assert!(session.data().await.is_ok());
            assert_eq!(
                session.set_new_password(s("short")).await,
                Err(CommandError::WeakPassword)
            );
            session
                .set_new_password(s("third password 3"))
                .await
                .unwrap();
            assert_eq!(
                session.set_new_password(s("fourth password 4")).await,
                Err(CommandError::NotAllowed),
                "only once, straight after the recovery unlock"
            );
            session.lock().await.unwrap();
            assert_eq!(session.data().await.err(), Some(CommandError::Locked));
            assert_eq!(
                session
                    .unlock(s("second password 2"), SecretKind::Password)
                    .await,
                Err(CommandError::UnlockFailed)
            );
            assert_eq!(
                session
                    .unlock(s("third password 3"), SecretKind::Password)
                    .await,
                Ok(false)
            );
            assert_eq!(
                session.set_new_password(s("fourth password 4")).await,
                Err(CommandError::NotAllowed)
            );
            assert_eq!(
                session
                    .regenerate_recovery_key(s("wrong password!!"))
                    .await
                    .err(),
                Some(CommandError::UnlockFailed)
            );
            let new_key = reveal_once(
                session
                    .regenerate_recovery_key(s("third password 3"))
                    .await
                    .unwrap(),
            );
            assert_ne!(new_key, key);
            session.lock().await.unwrap();
            assert_eq!(
                session.unlock(s(&key), SecretKind::RecoveryKey).await,
                Err(CommandError::UnlockFailed)
            );
            assert_eq!(
                session.unlock(s(&new_key), SecretKind::RecoveryKey).await,
                Ok(true)
            );
            session.lock().await.unwrap();
            session.lock().await.unwrap();
        });
    }
}
