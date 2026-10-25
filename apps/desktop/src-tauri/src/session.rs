//! The vault session behind the vault commands, kept free of Tauri so it can be
//! unit-tested. One lock guards everything: while an unlock (or its back-off
//! delay) runs, other vault commands answer `busy` instead of queueing.
//!
//! Secrets arrive as `String` from IPC; they are moved into wiped buffers at once
//! and never logged, returned or stored. The only secret that goes back to the
//! WebView is the recovery key, once, right after setup.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;
use shiftbook_core::ids::OwnerId;
use shiftbook_core::quickunlock::{valid_pin, window_open, PIN_TRIES};
use shiftbook_storage::backup::{
    self, backups_dir, BackupError, BackupFile, BackupKey, BackupKind, BackupSummary,
};
use shiftbook_storage::db::{database_path, open_encrypted};
use shiftbook_storage::repo::SettingsRepository;
use shiftbook_storage::{
    create_private_dir, delete_local_data, ExposeSecret, PinWrap, SecretString, SqliteStore,
    StorageError, Vault, VaultDir, VaultError,
};
use time::Date;
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
    /// Not a Shiftbook backup, or damaged.
    BackupInvalid,
    /// The backup was made with another key: ask for that password or recovery key.
    BackupNeedsSecret,
    /// The backup was made by a newer version of Shiftbook.
    BackupTooNew,
    /// Anything else (disk, database, randomness). Details stay in the app.
    StorageError,
}

impl From<BackupError> for CommandError {
    fn from(e: BackupError) -> Self {
        match e {
            BackupError::Invalid => CommandError::BackupInvalid,
            BackupError::NeedsSecret => CommandError::BackupNeedsSecret,
            BackupError::UnlockFailed => CommandError::UnlockFailed,
            BackupError::TooNew => CommandError::BackupTooNew,
            BackupError::Storage(e) => e.into(),
            BackupError::Io(_) | BackupError::Vault => CommandError::StorageError,
        }
    }
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
    fn for_backup(self) -> shiftbook_storage::vault::SecretKind {
        match self {
            SecretKind::Password => shiftbook_storage::vault::SecretKind::Password,
            SecretKind::RecoveryKey => shiftbook_storage::vault::SecretKind::RecoveryKey,
        }
    }

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

/// When the last full unlock (password, recovery key or setup) happened, by the app's own
/// clock and by the computer's. Quick unlock's window starts here.
#[derive(Clone, Copy)]
struct FullUnlock {
    app: Instant,
    wall: SystemTime,
}

impl FullUnlock {
    fn now() -> Self {
        Self {
            app: Instant::now(),
            wall: SystemTime::now(),
        }
    }

    /// Seconds since, by the app's clock and by the wall clock.
    fn elapsed(self) -> (u64, u64) {
        let wall = SystemTime::now()
            .duration_since(self.wall)
            .map_or(u64::MAX, |d| d.as_secs());
        (self.app.elapsed().as_secs(), wall)
    }
}

/// Quick unlock's PIN window (SECURITY.md "Quick unlock with a PIN"): the DEK wrapped under
/// the PIN, in memory only (wiped on drop), and the wrong PINs left.
struct PinWindow {
    wrap: PinWrap,
    since: FullUnlock,
    /// The local day it was set on; it ends at midnight.
    day: Date,
    tries_left: u8,
}

impl PinWindow {
    fn open(&self, today: Date) -> bool {
        let (app, wall) = self.since.elapsed();
        window_open(self.day, today, app, wall)
    }
}

/// What a PIN unlock did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinOutcome {
    /// Open now.
    pub unlocked: bool,
    /// Wrong PINs left; 0 means the window has ended and only the password works.
    pub tries_left: u8,
}

#[derive(Default)]
struct Inner {
    session: Option<Session>,
    backoff: Backoff,
    /// A backup file the user picked with the file dialog, until it is restored.
    picked_backup: Option<PathBuf>,
    /// The last full unlock, while the app runs.
    full_unlock: Option<FullUnlock>,
    /// Quick unlock's window, while it is open.
    pin: Option<PinWindow>,
}

/// The `source` that means "the file picked with the dialog".
pub const PICKED_BACKUP: &str = "picked";

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
        inner.full_unlock = Some(FullUnlock::now());
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
                // A new window starts; today's PIN is asked for again.
                inner.full_unlock = Some(FullUnlock::now());
                inner.pin = None;
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
                inner.pin = None;
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
        inner.pin = None;
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
                inner.pin = None;
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

impl VaultSession {
    /// Start today's PIN window: wrap the open vault's key under `pin` and keep that in memory
    /// until the window ends. Needs the app unlocked within the window of a full unlock.
    pub async fn start_quick_unlock(
        &self,
        pin: SecretString,
        today: Date,
    ) -> Result<(), CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        if !valid_pin(pin.expose_secret()) {
            return Err(CommandError::InvalidInput);
        }
        let since = inner.full_unlock.ok_or(CommandError::NotAllowed)?;
        let (app, wall) = since.elapsed();
        if !window_open(today, today, app, wall) {
            return Err(CommandError::NotAllowed);
        }
        let vault = Arc::clone(&session.vault);
        let vault_dir = self.vault_dir()?;
        let wrap = blocking(move || vault_dir.wrap_for_pin(&vault, &pin)).await??;
        inner.pin = Some(PinWindow {
            wrap,
            since,
            day: today,
            tries_left: PIN_TRIES,
        });
        Ok(())
    }

    /// Turn Quick unlock on from Settings: check the password, then start today's window.
    pub async fn enable_quick_unlock(
        &self,
        password: SecretString,
        pin: SecretString,
        today: Date,
    ) -> Result<(), CommandError> {
        {
            let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
            if inner.session.is_none() {
                return Err(CommandError::Locked);
            }
            if !valid_pin(pin.expose_secret()) {
                return Err(CommandError::InvalidInput);
            }
            let vault_dir = self.vault_dir()?;
            match blocking(move || vault_dir.unlock_with_password(&password)).await? {
                Ok(checked) => {
                    drop(checked);
                    inner.backoff.record_success();
                }
                Err(VaultError::UnlockFailed) => {
                    let delay = inner.backoff.record_failure();
                    Self::wait(delay).await;
                    return Err(CommandError::UnlockFailed);
                }
                Err(other) => return Err(other.into()),
            }
        }
        self.start_quick_unlock(pin, today).await
    }

    /// True when a PIN can open the app now (`today` is the local day).
    pub async fn quick_unlock_ready(&self, today: Date) -> Result<bool, CommandError> {
        let mut inner = self.inner.lock().await;
        let open = inner.pin.as_ref().is_some_and(|w| w.open(today));
        if !open {
            inner.pin = None;
        }
        Ok(open)
    }

    /// End the window now ("Use my password instead", a sleep the page saw, or turning Quick
    /// unlock off). The wrapped key is wiped.
    pub async fn end_quick_unlock(&self) -> Result<(), CommandError> {
        self.inner.lock().await.pin = None;
        Ok(())
    }

    /// Unlock with the PIN inside the window. A wrong PIN uses up one of the tries; after the
    /// last one the wrapped key is wiped and only the password or recovery key can open the
    /// app. Outside the window: `NotAllowed`.
    pub async fn unlock_with_pin(
        &self,
        pin: SecretString,
        today: Date,
    ) -> Result<PinOutcome, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        if inner.session.is_some() {
            return Ok(PinOutcome {
                unlocked: true,
                tries_left: PIN_TRIES,
            });
        }
        let window = inner.pin.take().ok_or(CommandError::NotAllowed)?;
        if !window.open(today) {
            return Err(CommandError::NotAllowed);
        }
        let (result, mut window) = blocking(move || {
            let result = if valid_pin(pin.expose_secret()) {
                window.wrap.unlock(&pin)
            } else {
                Err(VaultError::UnlockFailed)
            };
            (result, window)
        })
        .await?;
        match result {
            Ok(vault) => {
                inner.session = Some(self.open(vault).await?);
                window.tries_left = PIN_TRIES;
                inner.pin = Some(window);
                Ok(PinOutcome {
                    unlocked: true,
                    tries_left: PIN_TRIES,
                })
            }
            Err(VaultError::UnlockFailed) => {
                window.tries_left = window.tries_left.saturating_sub(1);
                let tries_left = window.tries_left;
                if tries_left > 0 {
                    inner.pin = Some(window);
                }
                Ok(PinOutcome {
                    unlocked: false,
                    tries_left,
                })
            }
            Err(other) => Err(other.into()),
        }
    }
}

impl VaultSession {
    /// Delete the database and the vault so the app starts again at setup. Needs
    /// only the typed confirmation word: it works while locked, for someone who has
    /// lost both the password and the recovery key. Closes the database first.
    pub async fn delete_all_data(&self, typed: &str) -> Result<(), CommandError> {
        if !shiftbook_core::wipe::confirms_delete(typed) {
            return Err(CommandError::InvalidInput);
        }
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let dir = self.data_dir()?;
        if let Some(session) = inner.session.take() {
            session.store.pool().close().await;
            drop(session);
        }
        inner.pin = None;
        inner.full_unlock = None;
        blocking(move || delete_local_data(&dir))
            .await?
            .map_err(|_| CommandError::StorageError)?;
        inner.backoff.record_success();
        Ok(())
    }
}

impl VaultSession {
    /// Make a backup of all data, as file bytes.
    async fn backup_bytes(&self, session: &Session) -> Result<Vec<u8>, CommandError> {
        Ok(backup::create_backup(
            &session.store,
            &self.vault_dir()?,
            session.vault.dek(),
            env!("CARGO_PKG_VERSION"),
            time::OffsetDateTime::now_utc(),
        )
        .await?)
    }

    /// Back up to a file the user chose.
    pub async fn backup_to(&self, path: PathBuf) -> Result<(), CommandError> {
        let inner = self.inner.lock().await;
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        let bytes = self.backup_bytes(session).await?;
        drop(inner);
        blocking(move || backup::save_at(&path, &bytes))
            .await?
            .map_err(|_| CommandError::StorageError)
    }

    /// Make the daily automatic backup if one is due. Returns true if it made one.
    pub async fn automatic_backup(&self) -> Result<bool, CommandError> {
        let dir = backups_dir(&self.data_dir()?);
        let now = time::OffsetDateTime::now_utc();
        let check = dir.clone();
        if !blocking(move || backup::automatic_due(&check, now))
            .await?
            .map_err(|_| CommandError::StorageError)?
        {
            return Ok(false);
        }
        let inner = self.inner.lock().await;
        let Some(session) = inner.session.as_ref() else {
            return Ok(false);
        };
        let bytes = self.backup_bytes(session).await?;
        drop(inner);
        blocking(move || backup::save_in_folder(&dir, BackupKind::Automatic, now, &bytes))
            .await?
            .map_err(|_| CommandError::StorageError)?;
        Ok(true)
    }

    /// The automatic and safety backups, newest first.
    pub async fn list_backups(&self) -> Result<Vec<BackupFile>, CommandError> {
        if self.inner.lock().await.session.is_none() {
            return Err(CommandError::Locked);
        }
        let dir = backups_dir(&self.data_dir()?);
        blocking(move || backup::list_backups(&dir))
            .await?
            .map_err(|_| CommandError::StorageError)
    }

    /// Remember a file the user picked, for [`PICKED_BACKUP`].
    pub async fn set_picked_backup(&self, path: PathBuf) -> Result<(), CommandError> {
        let mut inner = self.inner.lock().await;
        if inner.session.is_none() {
            return Err(CommandError::Locked);
        }
        inner.picked_backup = Some(path);
        Ok(())
    }

    fn source_path(&self, inner: &Inner, source: &str) -> Result<PathBuf, CommandError> {
        if source == PICKED_BACKUP {
            return inner.picked_backup.clone().ok_or(CommandError::NotFound);
        }
        backup::backup_path(&backups_dir(&self.data_dir()?), source).ok_or(CommandError::NotFound)
    }

    /// Read and decrypt a backup. With no secret it tries the key that is
    /// unlocked now (any backup made on this computer).
    async fn open_source(
        &self,
        inner: &mut Inner,
        source: &str,
        secret: Option<(SecretString, SecretKind)>,
    ) -> Result<backup::OpenedBackup, CommandError> {
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        let path = self.source_path(inner, source)?;
        let schema = backup::current_schema_version(&session.store).await?;
        let vault = Arc::clone(&session.vault);
        let vault_dir = self.vault_dir()?;
        let result = blocking(move || {
            let bytes = backup::read_backup_file(&path)?;
            let key = match &secret {
                None => BackupKey::Current(vault.dek()),
                Some((text, kind)) => BackupKey::Secret(text, kind.for_backup()),
            };
            backup::open_backup(&bytes, key, &vault_dir, schema)
        })
        .await?;
        match result {
            Ok(opened) => Ok(opened),
            Err(BackupError::UnlockFailed) => {
                let delay = inner.backoff.record_failure();
                Self::wait(delay).await;
                Err(CommandError::UnlockFailed)
            }
            Err(other) => Err(other.into()),
        }
    }

    /// Check a backup and say what it holds. Changes nothing.
    pub async fn check_backup(
        &self,
        source: &str,
        secret: Option<(SecretString, SecretKind)>,
    ) -> Result<BackupSummary, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        Ok(self
            .open_source(&mut inner, source, secret)
            .await?
            .summary())
    }

    /// Replace all data with a backup's: check it, save a safety backup of the
    /// current data, then replace everything in one transaction.
    pub async fn restore_backup(
        &self,
        source: &str,
        secret: Option<(SecretString, SecretKind)>,
    ) -> Result<BackupSummary, CommandError> {
        let mut inner = self.inner.try_lock().map_err(|_| CommandError::Busy)?;
        let opened = self.open_source(&mut inner, source, secret).await?;
        let session = inner.session.as_ref().ok_or(CommandError::Locked)?;
        let safety = self.backup_bytes(session).await?;
        let dir = backups_dir(&self.data_dir()?);
        let now = time::OffsetDateTime::now_utc();
        blocking(move || backup::save_in_folder(&dir, BackupKind::BeforeRestore, now, &safety))
            .await?
            .map_err(|_| CommandError::StorageError)?;
        backup::restore_backup(&session.store, &opened).await?;
        let user = session
            .store
            .initialize_local_owner(LOCAL_USER_NAME)
            .await?;
        if let Some(session) = inner.session.as_mut() {
            session.owner = user.id;
        }
        if source == PICKED_BACKUP {
            inner.picked_backup = None;
        }
        Ok(opened.summary())
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

    #[test]
    fn backups_restore_and_keep_a_safety_copy() {
        let dir = TempDir::new("backups");
        let session = VaultSession::new(Some(dir.0.clone()));
        block_on(async {
            assert_eq!(session.list_backups().await, Err(CommandError::Locked));
            session.setup(s("first password 1")).await.unwrap();
            let (store, owner) = session.data().await.unwrap();
            store.set_setting(owner, "marker", "kept").await.unwrap();

            assert_eq!(session.automatic_backup().await, Ok(true));
            assert_eq!(session.automatic_backup().await, Ok(false), "once a day");
            let list = session.list_backups().await.unwrap();
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].kind, BackupKind::Automatic);

            store.set_setting(owner, "marker", "changed").await.unwrap();
            let name = list[0].name.clone();
            let summary = session.check_backup(&name, None).await.unwrap();
            assert_eq!(summary.entries, 0);
            assert_eq!(
                session.check_backup("../vault.json", None).await,
                Err(CommandError::NotFound)
            );
            assert_eq!(
                session.check_backup(PICKED_BACKUP, None).await,
                Err(CommandError::NotFound)
            );

            session.restore_backup(&name, None).await.unwrap();
            let (store, owner) = session.data().await.unwrap();
            assert_eq!(
                store.get_setting(owner, "marker").await.unwrap().as_deref(),
                Some("kept")
            );
            let list = session.list_backups().await.unwrap();
            assert!(list.iter().any(|f| f.kind == BackupKind::BeforeRestore));

            // A file that isn't a backup changes nothing.
            let junk = dir.0.join("junk.shiftbak");
            std::fs::write(&junk, b"not a backup").unwrap();
            session.set_picked_backup(junk).await.unwrap();
            assert_eq!(
                session.restore_backup(PICKED_BACKUP, None).await,
                Err(CommandError::BackupInvalid)
            );
            session.lock().await.unwrap();
        });
    }

    #[test]
    fn delete_all_data_starts_again() {
        let dir = TempDir::new("delete");
        let session = VaultSession::new(Some(dir.0.clone()));
        block_on(async {
            session.setup(s("first password 1")).await.unwrap();
            assert_eq!(
                session.delete_all_data("yes").await,
                Err(CommandError::InvalidInput)
            );
            assert_eq!(session.status().await, Ok(VaultStatus::Unlocked));

            // While unlocked: closes the database, then deletes it.
            session.delete_all_data("delete").await.unwrap();
            assert_eq!(session.status().await, Ok(VaultStatus::NeedsSetup));
            assert_eq!(session.data().await.err(), Some(CommandError::Locked));
            assert!(!database_path(&dir.0).exists());

            // While locked, after failed unlocks: no password needed.
            session.setup(s("second password 2")).await.unwrap();
            session.lock().await.unwrap();
            assert_eq!(
                session
                    .unlock(s("forgotten password"), SecretKind::Password)
                    .await,
                Err(CommandError::UnlockFailed)
            );
            session.delete_all_data(" DELETE ").await.unwrap();
            assert_eq!(session.status().await, Ok(VaultStatus::NeedsSetup));

            // A fresh start opens a new, empty database.
            session.setup(s("third password 3")).await.unwrap();
            let (store, owner) = session.data().await.unwrap();
            assert!(store
                .get_setting(owner, RECOVERY_SAVED_KEY)
                .await
                .unwrap()
                .is_none());
            session.lock().await.unwrap();
        });
    }

    /// Quick unlock with a PIN: the tests SECURITY.md lists, with real Argon2 parameters.
    #[test]
    fn quick_unlock_with_a_pin() {
        let dir = TempDir::new("pin");
        let session = VaultSession::new(Some(dir.0.clone()));
        let mon = time::macros::date!(2026 - 10 - 19);
        let tue = time::macros::date!(2026 - 10 - 20);
        block_on(async {
            session.setup(s("first password 1")).await.unwrap();
            let vault_before = std::fs::read(dir.0.join("vault.json")).unwrap();

            // Turning it on needs the password and a 6-digit PIN.
            assert_eq!(
                session
                    .enable_quick_unlock(s("wrong password!!"), s("042917"), mon)
                    .await,
                Err(CommandError::UnlockFailed)
            );
            assert_eq!(
                session
                    .enable_quick_unlock(s("first password 1"), s("4291"), mon)
                    .await,
                Err(CommandError::InvalidInput)
            );
            session
                .enable_quick_unlock(s("first password 1"), s("042917"), mon)
                .await
                .unwrap();

            // Inside the window the PIN opens it, after Lock now too.
            session.lock().await.unwrap();
            assert_eq!(session.quick_unlock_ready(mon).await, Ok(true));
            let opened = session.unlock_with_pin(s("042917"), mon).await.unwrap();
            assert!(opened.unlocked);
            assert_eq!(session.status().await, Ok(VaultStatus::Unlocked));
            assert!(session.data().await.is_ok());

            // Nothing PIN-related on disk: vault.json is unchanged.
            assert_eq!(
                std::fs::read(dir.0.join("vault.json")).unwrap(),
                vault_before
            );

            // Three wrong PINs wipe the wrapped key: the right one then fails.
            session.lock().await.unwrap();
            for left in [2, 1, 0] {
                let wrong = session.unlock_with_pin(s("000000"), mon).await.unwrap();
                assert_eq!((wrong.unlocked, wrong.tries_left), (false, left));
            }
            assert_eq!(
                session.unlock_with_pin(s("042917"), mon).await,
                Err(CommandError::NotAllowed)
            );
            assert_eq!(session.quick_unlock_ready(mon).await, Ok(false));

            // A password unlock starts a new window; today's PIN must be set again.
            session
                .unlock(s("first password 1"), SecretKind::Password)
                .await
                .unwrap();
            assert_eq!(session.quick_unlock_ready(mon).await, Ok(false));
            session.start_quick_unlock(s("042917"), mon).await.unwrap();
            session.lock().await.unwrap();

            // After midnight it asks for the password.
            assert_eq!(
                session.unlock_with_pin(s("042917"), tue).await,
                Err(CommandError::NotAllowed)
            );

            // After a sleep (the wall clock ran on without the app) or 10 hours.
            for (app_back, wall_back) in [(0, 20 * 60), (10 * 3600, 10 * 3600)] {
                session
                    .unlock(s("first password 1"), SecretKind::Password)
                    .await
                    .unwrap();
                session.start_quick_unlock(s("042917"), mon).await.unwrap();
                session.lock().await.unwrap();
                {
                    let mut inner = session.inner.lock().await;
                    let w = inner.pin.as_mut().unwrap();
                    // A computer up for less than 10 hours can't go back that far; the
                    // wall clock still ends the window then.
                    w.since.app = Instant::now()
                        .checked_sub(Duration::from_secs(app_back))
                        .unwrap_or_else(Instant::now);
                    w.since.wall = SystemTime::now() - Duration::from_secs(wall_back);
                }
                assert_eq!(
                    session.unlock_with_pin(s("042917"), mon).await,
                    Err(CommandError::NotAllowed)
                );
            }

            // Changing the password ends the window, and "Use my password instead" too.
            session
                .unlock(s("first password 1"), SecretKind::Password)
                .await
                .unwrap();
            session.start_quick_unlock(s("042917"), mon).await.unwrap();
            session
                .change_password(s("first password 1"), s("second password 2"))
                .await
                .unwrap();
            assert_eq!(session.quick_unlock_ready(mon).await, Ok(false));
            session.start_quick_unlock(s("042917"), mon).await.unwrap();
            session.end_quick_unlock().await.unwrap();
            session.lock().await.unwrap();
            assert_eq!(
                session.unlock_with_pin(s("042917"), mon).await,
                Err(CommandError::NotAllowed)
            );
        });

        // Quitting the app ends it: a new session has no window.
        let again = VaultSession::new(Some(dir.0.clone()));
        block_on(async {
            assert_eq!(again.quick_unlock_ready(mon).await, Ok(false));
            assert_eq!(
                again.unlock_with_pin(s("042917"), mon).await,
                Err(CommandError::NotAllowed)
            );
        });
    }
}
