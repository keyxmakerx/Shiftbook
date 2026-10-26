//! The vault: the database key (DEK), wrapped by a key from the master password
//! and by a key from the recovery key. See SECURITY.md "Encryption design".
//!
//! - DEK: 256 random bits from the OS. It never changes, so changing the password
//!   only re-wraps it; the database is not re-encrypted.
//! - KEK: Argon2id(secret, 16-byte random salt), one salt per purpose. Parameters
//!   are calibrated at setup (~0.5 s) and stored in `vault.json`.
//! - Wrap: XChaCha20-Poly1305 with a fresh random 24-byte nonce. The AAD binds the
//!   vault version, KDF name and parameters, salt and purpose, so editing any of
//!   them makes the tag check fail. The tag check is the only password check.
//!
//! Every unlock failure (wrong secret, edited or damaged file, parameters out of
//! range) is the same [`VaultError::UnlockFailed`], so nothing reveals which
//! part was wrong.
//!
//! Quick unlock (round 15): [`PinWrap`] is one more wrap of the DEK, under a key from a
//! 6-digit PIN with the same Argon2id parameters. It lives in memory only and is wiped on
//! drop; it is never part of `vault.json` or a backup.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use secrecy::{ExposeSecret, ExposeSecretMut, SecretBox, SecretString};
use serde::{Deserialize, Serialize};
use shiftbook_core::validate::validate_new_password;
use shiftbook_core::CoreError;
use thiserror::Error;
use zeroize::Zeroizing;

use crate::fsutil;
use crate::hex;
use crate::recovery::RecoveryKey;

/// File name of the vault inside the data folder.
pub const VAULT_FILE_NAME: &str = "vault.json";
/// File name of the safety copy of the last good vault.
pub const VAULT_PREV_FILE_NAME: &str = "vault.json.prev";
/// The only vault format so far.
pub const VAULT_VERSION: u32 = 1;
/// The only key derivation function: Argon2id, version 0x13.
pub const KDF_NAME: &str = "argon2id";

const DEK_BYTES: usize = 32;
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;
const WRAPPED_BYTES: usize = DEK_BYTES + TAG_BYTES;
/// A real vault file is well under 1 KiB; refuse anything silly before parsing.
const MAX_VAULT_FILE_BYTES: u64 = 16 * 1024;

/// Argon2id parameters: memory in KiB, passes, lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_kib: u32,
    /// Time cost (passes over memory).
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

/// The weakest parameters ever accepted: 64 MiB, 3 passes, 1 lane.
pub const KDF_FLOOR: KdfParams = KdfParams {
    m_kib: 64 * 1024,
    t: 3,
    p: 1,
};

/// The strongest parameters accepted from a file (and the calibration ceiling for
/// memory): 1 GiB, 10 passes, 4 lanes. Anything above could be a denial of service.
pub const KDF_CAP: KdfParams = KdfParams {
    m_kib: 1024 * 1024,
    t: 10,
    p: 4,
};

/// The most memory calibration picks for a new vault. Phones share a few GiB with every other
/// app and Android stops apps that take too much, so a phone stops at 256 MiB and adds passes
/// instead; a computer can go up to the cap.
#[cfg(target_os = "android")]
const CALIBRATION_MAX_M_KIB: u32 = 256 * 1024;
#[cfg(not(target_os = "android"))]
const CALIBRATION_MAX_M_KIB: u32 = KDF_CAP.m_kib;

/// How long a key derivation should take on this computer.
pub const KDF_TARGET: Duration = Duration::from_millis(500);

impl KdfParams {
    /// True when every value is within `min..=max`.
    fn within(&self, min: &KdfParams, max: &KdfParams) -> bool {
        (min.m_kib..=max.m_kib).contains(&self.m_kib)
            && (min.t..=max.t).contains(&self.t)
            && (min.p..=max.p).contains(&self.p)
            // Argon2 needs at least 8 KiB per lane.
            && self.m_kib >= 8 * self.p
    }
}

/// Pick parameters so one derivation takes about `target`, given that the floor
/// took `floor_took`. Memory grows first (up to [`CALIBRATION_MAX_M_KIB`]), then passes.
pub fn calibrated_params(floor_took: Duration, target: Duration) -> KdfParams {
    let took = floor_took.as_secs_f64().max(0.001);
    let factor = target.as_secs_f64() / took;
    if factor <= 1.0 {
        return KDF_FLOOR;
    }
    let want_m = f64::from(KDF_FLOOR.m_kib) * factor;
    // Whole MiB, between the floor and the cap.
    let m_kib = ((want_m / 1024.0).floor() * 1024.0)
        .clamp(f64::from(KDF_FLOOR.m_kib), f64::from(CALIBRATION_MAX_M_KIB)) as u32;
    let left = want_m / f64::from(m_kib);
    let t = (f64::from(KDF_FLOOR.t) * left)
        .floor()
        .clamp(f64::from(KDF_FLOOR.t), f64::from(KDF_CAP.t)) as u32;
    KdfParams {
        m_kib,
        t,
        p: KDF_FLOOR.p,
    }
}

/// How new vaults choose their parameters and which parameters are accepted.
#[derive(Debug, Clone, Copy)]
enum KdfPolicy {
    /// Real use: calibrate on this computer; accept only floor..=cap.
    Production,
    /// Unit tests only: tiny fixed parameters so tests are fast. This variant
    /// does not exist outside `cfg(test)`, so a release build can't use it.
    #[cfg(test)]
    Test,
}

#[cfg(test)]
const TEST_PARAMS: KdfParams = KdfParams {
    m_kib: 64,
    t: 1,
    p: 1,
};

impl KdfPolicy {
    fn accepts(self, params: &KdfParams) -> bool {
        match self {
            KdfPolicy::Production => params.within(&KDF_FLOOR, &KDF_CAP),
            #[cfg(test)]
            KdfPolicy::Test => params.within(&TEST_PARAMS, &KDF_CAP),
        }
    }

    fn choose(self) -> Result<KdfParams, VaultError> {
        match self {
            KdfPolicy::Production => {
                let started = Instant::now();
                let probe = derive_kek(b"calibration", &[0u8; SALT_BYTES], &KDF_FLOOR)?;
                drop(probe);
                Ok(calibrated_params(started.elapsed(), KDF_TARGET))
            }
            #[cfg(test)]
            KdfPolicy::Test => Ok(TEST_PARAMS),
        }
    }
}

/// Anything that can go wrong with the vault. Messages are plain and never contain
/// a secret; unlock problems are all the same variant.
#[derive(Debug, Error)]
pub enum VaultError {
    /// Wrong password or recovery key, or the vault file was changed or damaged.
    #[error("Couldn't unlock. Check your password or recovery key and try again.")]
    UnlockFailed,
    /// The new password doesn't meet the rules.
    #[error("{0}")]
    WeakPassword(CoreError),
    /// Setup was asked for, but a vault already exists.
    #[error("Shiftbook is already set up in this folder.")]
    AlreadySetUp,
    /// Unlock was asked for, but there is no vault yet.
    #[error("Shiftbook hasn't been set up yet.")]
    NotSetUp,
    /// The vault file couldn't be read or written.
    #[error("The key file couldn't be read or saved ({0}).")]
    Io(io::ErrorKind),
    /// The OS random number generator failed.
    #[error("The system couldn't provide secure random numbers.")]
    Random,
    /// Key derivation failed (for example, not enough memory).
    #[error("The key couldn't be prepared on this computer.")]
    Kdf,
}

impl From<io::Error> for VaultError {
    fn from(e: io::Error) -> Self {
        VaultError::Io(e.kind())
    }
}

/// The database key. Wiped from memory when dropped; no `Debug`, `Clone` or
/// `Serialize`. Only this crate can read the bytes.
pub struct Dek(SecretBox<[u8; DEK_BYTES]>);

impl Dek {
    fn generate() -> Result<Self, VaultError> {
        let mut key = SecretBox::new(Box::new([0u8; DEK_BYTES]));
        random_fill(key.expose_secret_mut())?;
        Ok(Self(key))
    }

    pub(crate) fn expose(&self) -> &[u8; DEK_BYTES] {
        self.0.expose_secret()
    }

    #[cfg(test)]
    pub(crate) fn from_bytes_for_tests(bytes: [u8; DEK_BYTES]) -> Self {
        Self(SecretBox::new(Box::new(bytes)))
    }
}

/// An unlocked vault: holds the database key until dropped (then wiped).
pub struct Vault {
    dek: Dek,
}

impl Vault {
    /// The database key, for [`crate::db::open_encrypted`].
    pub fn dek(&self) -> &Dek {
        &self.dek
    }
}

/// The DEK wrapped under a PIN for Quick unlock: in memory only, wiped when dropped. No
/// `Debug`, `Clone` or `Serialize`.
pub struct PinWrap {
    params: KdfParams,
    salt: Zeroizing<[u8; SALT_BYTES]>,
    nonce: [u8; NONCE_BYTES],
    wrapped: Zeroizing<[u8; WRAPPED_BYTES]>,
}

impl PinWrap {
    /// Unwrap the DEK with `pin`; a wrong PIN is [`VaultError::UnlockFailed`].
    pub fn unlock(&self, pin: &SecretString) -> Result<Vault, VaultError> {
        let kek = derive_kek(pin.expose_secret().as_bytes(), &self.salt, &self.params)?;
        let cipher = XChaCha20Poly1305::new_from_slice(kek.as_ref())
            .map_err(|_| VaultError::UnlockFailed)?;
        let plain = Zeroizing::new(
            cipher
                .decrypt(
                    &XNonce::from(self.nonce),
                    Payload {
                        msg: self.wrapped.as_ref(),
                        aad: &aad_for(&self.params, &self.salt, PIN_LABEL),
                    },
                )
                .map_err(|_| VaultError::UnlockFailed)?,
        );
        if plain.len() != DEK_BYTES {
            return Err(VaultError::UnlockFailed);
        }
        let mut key = SecretBox::new(Box::new([0u8; DEK_BYTES]));
        key.expose_secret_mut().copy_from_slice(&plain);
        Ok(Vault { dek: Dek(key) })
    }
}

/// The vault files in one data folder.
#[derive(Debug, Clone)]
pub struct VaultDir {
    dir: PathBuf,
    policy: KdfPolicy,
}

/// Whether a password or a recovery key is being used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    Password,
    Recovery,
}

impl Purpose {
    fn label(self) -> &'static str {
        match self {
            Purpose::Password => "pw",
            Purpose::Recovery => "recovery",
        }
    }
}

/// `vault.json` exactly as stored. Bytes are lowercase hex.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultFile {
    version: u32,
    kdf: String,
    kdf_params: KdfParams,
    salts: PerPurpose,
    nonces: PerPurpose,
    wrapped_dek_password: String,
    wrapped_dek_recovery: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerPurpose {
    password: String,
    recovery: String,
}

/// One wrapped copy of the DEK.
#[derive(Clone, Copy)]
struct Wrap {
    salt: [u8; SALT_BYTES],
    nonce: [u8; NONCE_BYTES],
    wrapped: [u8; WRAPPED_BYTES],
}

/// A parsed, range-checked vault.
#[derive(Clone, Copy)]
struct VaultData {
    params: KdfParams,
    password: Wrap,
    recovery: Wrap,
}

impl VaultData {
    fn parse(bytes: &[u8], policy: KdfPolicy) -> Result<Self, VaultError> {
        let file: VaultFile =
            serde_json::from_slice(bytes).map_err(|_| VaultError::UnlockFailed)?;
        if file.version != VAULT_VERSION
            || file.kdf != KDF_NAME
            || !policy.accepts(&file.kdf_params)
        {
            return Err(VaultError::UnlockFailed);
        }
        let wrap = |salt: &str, nonce: &str, wrapped: &str| -> Option<Wrap> {
            Some(Wrap {
                salt: hex::decode_array(salt)?,
                nonce: hex::decode_array(nonce)?,
                wrapped: hex::decode_array(wrapped)?,
            })
        };
        let password = wrap(
            &file.salts.password,
            &file.nonces.password,
            &file.wrapped_dek_password,
        )
        .ok_or(VaultError::UnlockFailed)?;
        let recovery = wrap(
            &file.salts.recovery,
            &file.nonces.recovery,
            &file.wrapped_dek_recovery,
        )
        .ok_or(VaultError::UnlockFailed)?;
        Ok(Self {
            params: file.kdf_params,
            password,
            recovery,
        })
    }

    fn to_json(self) -> Result<Vec<u8>, VaultError> {
        let file = VaultFile {
            version: VAULT_VERSION,
            kdf: KDF_NAME.to_string(),
            kdf_params: self.params,
            salts: PerPurpose {
                password: hex::encode(&self.password.salt),
                recovery: hex::encode(&self.recovery.salt),
            },
            nonces: PerPurpose {
                password: hex::encode(&self.password.nonce),
                recovery: hex::encode(&self.recovery.nonce),
            },
            wrapped_dek_password: hex::encode(&self.password.wrapped),
            wrapped_dek_recovery: hex::encode(&self.recovery.wrapped),
        };
        let mut json = serde_json::to_vec_pretty(&file).map_err(|_| VaultError::Kdf)?;
        json.push(b'\n');
        Ok(json)
    }

    fn wrap_for(&self, purpose: Purpose) -> &Wrap {
        match purpose {
            Purpose::Password => &self.password,
            Purpose::Recovery => &self.recovery,
        }
    }
}

fn random_fill(buf: &mut [u8]) -> Result<(), VaultError> {
    getrandom::fill(buf).map_err(|_| VaultError::Random)
}

fn random_array<const N: usize>() -> Result<[u8; N], VaultError> {
    let mut out = [0u8; N];
    random_fill(&mut out)?;
    Ok(out)
}

/// Argon2id(secret, salt) → 32-byte key, wiped when dropped.
fn derive_kek(
    secret: &[u8],
    salt: &[u8; SALT_BYTES],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; 32]>, VaultError> {
    let params =
        Params::new(params.m_kib, params.t, params.p, Some(32)).map_err(|_| VaultError::Kdf)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(secret, salt, out.as_mut())
        .map_err(|_| VaultError::Kdf)?;
    Ok(out)
}

/// Associated data for a wrap: everything an attacker might edit in the file
/// except the nonce (which is the cipher's own input).
fn aad(params: &KdfParams, salt: &[u8; SALT_BYTES], purpose: Purpose) -> Vec<u8> {
    aad_for(params, salt, purpose.label())
}

/// The PIN wrap's purpose label in its associated data.
const PIN_LABEL: &str = "pin";

fn aad_for(params: &KdfParams, salt: &[u8; SALT_BYTES], label: &str) -> Vec<u8> {
    format!(
        "shiftbook-vault|version={VAULT_VERSION}|kdf={KDF_NAME}-v19|m={}|t={}|p={}|purpose={}|salt={}",
        params.m_kib,
        params.t,
        params.p,
        label,
        hex::encode(salt)
    )
    .into_bytes()
}

/// Wrap the DEK for `purpose` with a new salt and a new nonce.
fn wrap_dek(
    dek: &Dek,
    secret: &[u8],
    params: &KdfParams,
    purpose: Purpose,
) -> Result<Wrap, VaultError> {
    let salt = random_array::<SALT_BYTES>()?;
    let nonce = random_array::<NONCE_BYTES>()?;
    let kek = derive_kek(secret, &salt, params)?;
    let cipher = XChaCha20Poly1305::new_from_slice(kek.as_ref()).map_err(|_| VaultError::Kdf)?;
    let ciphertext = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: dek.expose(),
                aad: &aad(params, &salt, purpose),
            },
        )
        .map_err(|_| VaultError::Kdf)?;
    let wrapped =
        <[u8; WRAPPED_BYTES]>::try_from(ciphertext.as_slice()).map_err(|_| VaultError::Kdf)?;
    Ok(Wrap {
        salt,
        nonce,
        wrapped,
    })
}

/// Unwrap the DEK; any failure is [`VaultError::UnlockFailed`].
fn unwrap_dek(data: &VaultData, secret: &[u8], purpose: Purpose) -> Result<Dek, VaultError> {
    let wrap = data.wrap_for(purpose);
    let kek = derive_kek(secret, &wrap.salt, &data.params)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(kek.as_ref()).map_err(|_| VaultError::UnlockFailed)?;
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                &XNonce::from(wrap.nonce),
                Payload {
                    msg: &wrap.wrapped,
                    aad: &aad(&data.params, &wrap.salt, purpose),
                },
            )
            .map_err(|_| VaultError::UnlockFailed)?,
    );
    if plain.len() != DEK_BYTES {
        return Err(VaultError::UnlockFailed);
    }
    let mut key = SecretBox::new(Box::new([0u8; DEK_BYTES]));
    key.expose_secret_mut().copy_from_slice(&plain);
    Ok(Dek(key))
}

impl VaultDir {
    /// The vault in `dir` (the app data folder).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            policy: KdfPolicy::Production,
        }
    }

    /// Unit tests only: tiny Argon2 parameters so tests run fast.
    #[cfg(test)]
    pub(crate) fn new_for_tests(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            policy: KdfPolicy::Test,
        }
    }

    /// The folder holding the vault (and the database).
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Path of `vault.json`.
    pub fn vault_path(&self) -> PathBuf {
        self.dir.join(VAULT_FILE_NAME)
    }

    fn prev_path(&self) -> PathBuf {
        self.dir.join(VAULT_PREV_FILE_NAME)
    }

    /// True once setup has written `vault.json`.
    pub fn exists(&self) -> bool {
        self.vault_path().is_file()
    }

    fn load(&self) -> Result<VaultData, VaultError> {
        let path = self.vault_path();
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(VaultError::NotSetUp),
            Err(e) => return Err(e.into()),
        };
        if meta.len() > MAX_VAULT_FILE_BYTES {
            return Err(VaultError::UnlockFailed);
        }
        let bytes = fs::read(&path)?;
        VaultData::parse(&bytes, self.policy)
    }

    /// Save a new vault: keep the current file as `vault.json.prev`, replace
    /// `vault.json` atomically, read it back, then make `.prev` a copy of the new
    /// file so an old password doesn't keep working through the safety copy.
    fn save(&self, data: VaultData) -> Result<(), VaultError> {
        fsutil::create_private_dir(&self.dir)?;
        let json = data.to_json()?;
        let path = self.vault_path();
        let prev = self.prev_path();
        match fs::read(&path) {
            Ok(old) => fsutil::write_atomic(&prev, &old)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        fsutil::write_atomic(&path, &json)?;
        if fs::read(&path)? != json {
            return Err(VaultError::Io(io::ErrorKind::InvalidData));
        }
        fsutil::write_atomic(&prev, &json)?;
        Ok(())
    }

    /// First-run setup: new DEK, calibrated parameters, password and recovery
    /// wraps. Returns the unlocked vault and the recovery key to show once.
    pub fn create(&self, password: &SecretString) -> Result<(Vault, RecoveryKey), VaultError> {
        validate_new_password(password.expose_secret()).map_err(VaultError::WeakPassword)?;
        if self.vault_path().exists() {
            return Err(VaultError::AlreadySetUp);
        }
        let params = self.policy.choose()?;
        let dek = Dek::generate()?;
        let recovery_key = RecoveryKey::generate().map_err(|_| VaultError::Random)?;
        let data = VaultData {
            params,
            password: wrap_dek(
                &dek,
                password.expose_secret().as_bytes(),
                &params,
                Purpose::Password,
            )?,
            recovery: wrap_dek(&dek, recovery_key.as_bytes(), &params, Purpose::Recovery)?,
        };
        self.save(data)?;
        Ok((Vault { dek }, recovery_key))
    }

    /// Unlock with the master password.
    pub fn unlock_with_password(&self, password: &SecretString) -> Result<Vault, VaultError> {
        let data = self.load()?;
        let dek = unwrap_dek(
            &data,
            password.expose_secret().as_bytes(),
            Purpose::Password,
        )?;
        Ok(Vault { dek })
    }

    /// Wrap the open vault's DEK under `pin` (fresh salt and nonce, the vault's own Argon2id
    /// parameters) for Quick unlock. The result is kept in memory only.
    pub fn wrap_for_pin(&self, vault: &Vault, pin: &SecretString) -> Result<PinWrap, VaultError> {
        let params = self.load()?.params;
        let salt = Zeroizing::new(random_array::<SALT_BYTES>()?);
        let nonce = random_array::<NONCE_BYTES>()?;
        let kek = derive_kek(pin.expose_secret().as_bytes(), &salt, &params)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(kek.as_ref()).map_err(|_| VaultError::Kdf)?;
        let ciphertext = Zeroizing::new(
            cipher
                .encrypt(
                    &XNonce::from(nonce),
                    Payload {
                        msg: vault.dek.expose(),
                        aad: &aad_for(&params, &salt, PIN_LABEL),
                    },
                )
                .map_err(|_| VaultError::Kdf)?,
        );
        let mut wrapped = Zeroizing::new([0u8; WRAPPED_BYTES]);
        if ciphertext.len() != WRAPPED_BYTES {
            return Err(VaultError::Kdf);
        }
        wrapped.copy_from_slice(&ciphertext);
        Ok(PinWrap {
            params,
            salt,
            nonce,
            wrapped,
        })
    }

    /// Unlock with the recovery key as typed by the user.
    pub fn unlock_with_recovery_key(&self, typed: &SecretString) -> Result<Vault, VaultError> {
        let data = self.load()?;
        let key = RecoveryKey::parse(typed.expose_secret()).ok_or(VaultError::UnlockFailed)?;
        let dek = unwrap_dek(&data, key.as_bytes(), Purpose::Recovery)?;
        Ok(Vault { dek })
    }

    /// Change the master password. The current password is checked first (by
    /// unwrapping the DEK); only the password wrap changes. The recovery key keeps
    /// working.
    pub fn change_password(
        &self,
        current: &SecretString,
        new: &SecretString,
    ) -> Result<(), VaultError> {
        validate_new_password(new.expose_secret()).map_err(VaultError::WeakPassword)?;
        let mut data = self.load()?;
        let dek = unwrap_dek(&data, current.expose_secret().as_bytes(), Purpose::Password)?;
        data.password = wrap_dek(
            &dek,
            new.expose_secret().as_bytes(),
            &data.params,
            Purpose::Password,
        )?;
        self.save(data)
    }

    /// Set a new password with the key of a vault that is already unlocked, for
    /// when it was unlocked with the recovery key and the old password is
    /// forgotten (the owner decided to ask for a new one straight away). Only the
    /// password wrap changes; the recovery key keeps working. The caller must only
    /// offer this right after a recovery-key unlock of this same folder.
    pub fn reset_password(&self, vault: &Vault, new: &SecretString) -> Result<(), VaultError> {
        validate_new_password(new.expose_secret()).map_err(VaultError::WeakPassword)?;
        let mut data = self.load()?;
        data.password = wrap_dek(
            vault.dek(),
            new.expose_secret().as_bytes(),
            &data.params,
            Purpose::Password,
        )?;
        self.save(data)
    }

    /// Make a new recovery key (needs the password). The old key stops working.
    pub fn regenerate_recovery_key(
        &self,
        password: &SecretString,
    ) -> Result<RecoveryKey, VaultError> {
        let mut data = self.load()?;
        let dek = unwrap_dek(
            &data,
            password.expose_secret().as_bytes(),
            Purpose::Password,
        )?;
        let key = RecoveryKey::generate().map_err(|_| VaultError::Random)?;
        data.recovery = wrap_dek(&dek, key.as_bytes(), &data.params, Purpose::Recovery)?;
        self.save(data)?;
        Ok(key)
    }
}

/// Which secret opens a vault copied into a backup file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKind {
    /// The master password at the time of the backup.
    Password,
    /// The recovery key at the time of the backup.
    RecoveryKey,
}

impl VaultDir {
    /// The current vault, checked and re-encoded, for copying into a backup so the
    /// backup can be opened on another computer with the password or recovery key.
    pub(crate) fn vault_for_backup(&self) -> Result<Vec<u8>, VaultError> {
        self.load()?.to_json()
    }

    /// Unwrap the DEK of a vault copied into a backup. Any failure is
    /// [`VaultError::UnlockFailed`], as for the real vault.
    pub(crate) fn unlock_copied_vault(
        &self,
        vault_json: &[u8],
        secret: &SecretString,
        kind: SecretKind,
    ) -> Result<Dek, VaultError> {
        let data = VaultData::parse(vault_json, self.policy)?;
        match kind {
            SecretKind::Password => {
                unwrap_dek(&data, secret.expose_secret().as_bytes(), Purpose::Password)
            }
            SecretKind::RecoveryKey => {
                let key =
                    RecoveryKey::parse(secret.expose_secret()).ok_or(VaultError::UnlockFailed)?;
                unwrap_dek(&data, key.as_bytes(), Purpose::Recovery)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fsutil::test_dir::TestDir;

    fn secret(s: &str) -> SecretString {
        SecretString::from(s)
    }

    const PW: &str = "correct horse battery";

    fn setup(label: &str) -> (TestDir, VaultDir, Vault, RecoveryKey) {
        let dir = TestDir::new(label);
        let vault_dir = VaultDir::new_for_tests(dir.path());
        let (vault, key) = vault_dir.create(&secret(PW)).unwrap();
        (dir, vault_dir, vault, key)
    }

    fn read_json(v: &VaultDir) -> serde_json::Value {
        serde_json::from_slice(&fs::read(v.vault_path()).unwrap()).unwrap()
    }

    fn write_json(v: &VaultDir, value: &serde_json::Value) {
        fs::write(v.vault_path(), serde_json::to_vec(value).unwrap()).unwrap();
    }

    #[test]
    fn create_then_unlock_with_password() {
        let (_d, v, vault, _key) = setup("create");
        assert!(v.exists());
        let again = v.unlock_with_password(&secret(PW)).unwrap();
        assert_eq!(again.dek().expose(), vault.dek().expose());
        assert_ne!(vault.dek().expose(), &[0u8; 32]);
    }

    #[test]
    fn file_has_the_documented_shape() {
        let (_d, v, _vault, _key) = setup("shape");
        let json = read_json(&v);
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "kdf",
                "kdf_params",
                "nonces",
                "salts",
                "version",
                "wrapped_dek_password",
                "wrapped_dek_recovery"
            ]
        );
        assert_eq!(json["version"], 1);
        assert_eq!(json["kdf"], "argon2id");
        assert_eq!(json["salts"]["password"].as_str().unwrap().len(), 32);
        assert_eq!(json["nonces"]["recovery"].as_str().unwrap().len(), 48);
        assert_eq!(json["wrapped_dek_password"].as_str().unwrap().len(), 96);
        assert_ne!(json["salts"]["password"], json["salts"]["recovery"]);
        assert_ne!(json["nonces"]["password"], json["nonces"]["recovery"]);
        // The safety copy matches.
        assert_eq!(
            fs::read(v.vault_path()).unwrap(),
            fs::read(v.prev_path()).unwrap()
        );
    }

    #[test]
    fn wrong_password_fails_generically() {
        let (_d, v, _vault, _key) = setup("wrong");
        for wrong in ["correct horse battery ", "Correct horse battery", "", "x"] {
            assert!(matches!(
                v.unlock_with_password(&secret(wrong)),
                Err(VaultError::UnlockFailed)
            ));
        }
        let message = VaultError::UnlockFailed.to_string();
        assert!(message.starts_with("Couldn't unlock"));
    }

    #[test]
    fn recovery_key_unlocks() {
        let (_d, v, vault, key) = setup("recovery");
        let shown = key.display();
        let back = v.unlock_with_recovery_key(&shown).unwrap();
        assert_eq!(back.dek().expose(), vault.dek().expose());
        // Lower case, spaces instead of hyphens.
        let typed = secret(&shown.expose_secret().to_lowercase().replace('-', " "));
        assert!(v.unlock_with_recovery_key(&typed).is_ok());
        // The password is not a recovery key and vice versa.
        assert!(matches!(
            v.unlock_with_recovery_key(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        assert!(matches!(
            v.unlock_with_password(&shown),
            Err(VaultError::UnlockFailed)
        ));
        // A well-formed but different key fails.
        let other = RecoveryKey::generate().unwrap().display();
        assert!(matches!(
            v.unlock_with_recovery_key(&other),
            Err(VaultError::UnlockFailed)
        ));
    }

    #[test]
    fn change_password() {
        let (_d, v, vault, key) = setup("change");
        let new = "a brand new passphrase";
        assert!(matches!(
            v.change_password(&secret("not the password"), &secret(new)),
            Err(VaultError::UnlockFailed)
        ));
        assert!(matches!(
            v.change_password(&secret(PW), &secret("short")),
            Err(VaultError::WeakPassword(CoreError::PasswordTooShort { .. }))
        ));
        let before = read_json(&v);
        v.change_password(&secret(PW), &secret(new)).unwrap();
        let after = read_json(&v);
        assert_ne!(before["salts"]["password"], after["salts"]["password"]);
        assert_eq!(
            before["wrapped_dek_recovery"],
            after["wrapped_dek_recovery"]
        );
        assert!(matches!(
            v.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        let unlocked = v.unlock_with_password(&secret(new)).unwrap();
        assert_eq!(unlocked.dek().expose(), vault.dek().expose());
        let via_recovery = v.unlock_with_recovery_key(&key.display()).unwrap();
        assert_eq!(via_recovery.dek().expose(), vault.dek().expose());
        // The safety copy doesn't keep the old password working.
        let prev = VaultData::parse(&fs::read(v.prev_path()).unwrap(), v.policy).unwrap();
        assert!(unwrap_dek(&prev, PW.as_bytes(), Purpose::Password).is_err());
    }

    #[test]
    fn reset_password_after_recovery_unlock() {
        let (_d, v, vault, key) = setup("reset");
        let via_recovery = v.unlock_with_recovery_key(&key.display()).unwrap();
        assert!(matches!(
            v.reset_password(&via_recovery, &secret("short")),
            Err(VaultError::WeakPassword(_))
        ));
        v.reset_password(&via_recovery, &secret("a forgotten-then-new one"))
            .unwrap();
        assert!(matches!(
            v.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        let again = v
            .unlock_with_password(&secret("a forgotten-then-new one"))
            .unwrap();
        assert_eq!(again.dek().expose(), vault.dek().expose());
        assert!(v.unlock_with_recovery_key(&key.display()).is_ok());
    }

    #[test]
    fn regenerate_recovery_key() {
        let (_d, v, vault, old_key) = setup("regen");
        assert!(matches!(
            v.regenerate_recovery_key(&secret("wrong password!")),
            Err(VaultError::UnlockFailed)
        ));
        let new_key = v.regenerate_recovery_key(&secret(PW)).unwrap();
        assert!(matches!(
            v.unlock_with_recovery_key(&old_key.display()),
            Err(VaultError::UnlockFailed)
        ));
        let unlocked = v.unlock_with_recovery_key(&new_key.display()).unwrap();
        assert_eq!(unlocked.dek().expose(), vault.dek().expose());
        assert!(v.unlock_with_password(&secret(PW)).is_ok());
    }

    #[test]
    fn setup_rules() {
        let dir = TestDir::new("rules");
        let v = VaultDir::new_for_tests(dir.path());
        assert!(matches!(
            v.unlock_with_password(&secret(PW)),
            Err(VaultError::NotSetUp)
        ));
        assert!(matches!(
            v.create(&secret("elevenchars")),
            Err(VaultError::WeakPassword(_))
        ));
        assert!(!v.exists());
        v.create(&secret(PW)).unwrap();
        assert!(matches!(
            v.create(&secret(PW)),
            Err(VaultError::AlreadySetUp)
        ));
    }

    #[test]
    fn every_tampered_field_fails() {
        let (_d, v, _vault, key) = setup("tamper");
        let original = read_json(&v);
        let flip = |hex_text: &str| -> String {
            let mut bytes = hex::decode(hex_text).unwrap();
            bytes[0] ^= 0x01;
            hex::encode(&bytes)
        };
        let mut cases: Vec<(&str, serde_json::Value)> = Vec::new();
        let mut c = original.clone();
        c["version"] = 2.into();
        cases.push(("version", c));
        let mut c = original.clone();
        c["kdf"] = "argon2i".into();
        cases.push(("kdf", c));
        for field in ["m_kib", "t", "p"] {
            let mut c = original.clone();
            let n = c["kdf_params"][field].as_u64().unwrap();
            c["kdf_params"][field] = (n + 1).into();
            cases.push((field, c));
        }
        for group in ["salts", "nonces"] {
            for purpose in ["password", "recovery"] {
                let mut c = original.clone();
                let flipped = flip(c[group][purpose].as_str().unwrap());
                c[group][purpose] = flipped.into();
                cases.push(("salt/nonce", c));
            }
        }
        for field in ["wrapped_dek_password", "wrapped_dek_recovery"] {
            let mut c = original.clone();
            let flipped = flip(c[field].as_str().unwrap());
            c[field] = flipped.into();
            cases.push((field, c));
            // Truncated.
            let mut c = original.clone();
            let short = c[field].as_str().unwrap()[2..].to_string();
            c[field] = short.into();
            cases.push((field, c));
        }
        let mut c = original.clone();
        c["extra"] = "field".into();
        cases.push(("unknown field", c));
        let mut c = original.clone();
        c.as_object_mut().unwrap().remove("salts");
        cases.push(("missing field", c));

        for (what, value) in cases {
            write_json(&v, &value);
            let pw = v.unlock_with_password(&secret(PW));
            let rk = v.unlock_with_recovery_key(&key.display());
            // Each edit must break at least the wrap it touches; anything that is
            // shared (version, kdf, params, shape) must break both.
            let pw_failed = matches!(pw, Err(VaultError::UnlockFailed));
            let rk_failed = matches!(rk, Err(VaultError::UnlockFailed));
            assert!(pw_failed || rk_failed, "{what} was not detected");
            if !what.contains("salt") && !what.starts_with("wrapped") {
                assert!(pw_failed && rk_failed, "{what} must break both");
            }
        }
        // Garbage and an oversized file fail the same way.
        fs::write(v.vault_path(), b"not json").unwrap();
        assert!(matches!(
            v.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        fs::write(v.vault_path(), vec![b' '; 20_000]).unwrap();
        assert!(matches!(
            v.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        // Restoring the original works again.
        write_json(&v, &original);
        assert!(v.unlock_with_password(&secret(PW)).is_ok());
    }

    #[test]
    fn tampered_salt_or_wrap_breaks_only_that_purpose() {
        let (_d, v, _vault, key) = setup("tamper-one");
        let original = read_json(&v);
        let mut c = original.clone();
        let mut salt = hex::decode(c["salts"]["password"].as_str().unwrap()).unwrap();
        salt[3] ^= 0x80;
        c["salts"]["password"] = hex::encode(&salt).into();
        write_json(&v, &c);
        assert!(v.unlock_with_password(&secret(PW)).is_err());
        assert!(v.unlock_with_recovery_key(&key.display()).is_ok());
    }

    #[test]
    fn out_of_range_params_are_refused() {
        let floor = KDF_FLOOR;
        let accepts = |p: KdfParams| KdfPolicy::Production.accepts(&p);
        assert!(accepts(floor));
        assert!(accepts(KDF_CAP));
        assert!(!accepts(KdfParams {
            m_kib: floor.m_kib - 1,
            ..floor
        }));
        assert!(!accepts(KdfParams { t: 2, ..floor }));
        assert!(!accepts(KdfParams { p: 0, ..floor }));
        assert!(!accepts(KdfParams {
            m_kib: KDF_CAP.m_kib + 1,
            ..floor
        }));
        assert!(!accepts(KdfParams { t: 11, ..floor }));
        assert!(!accepts(KdfParams { p: 5, ..floor }));
        assert!(!accepts(KdfParams {
            m_kib: u32::MAX,
            t: u32::MAX,
            p: u32::MAX
        }));

        // A real (production) vault reader refuses a file with weak parameters,
        // even though the file is otherwise valid.
        let (dir, test_v, _vault, _key) = setup("weak-params");
        let prod = VaultDir::new(dir.path());
        assert!(matches!(
            prod.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        assert!(test_v.unlock_with_password(&secret(PW)).is_ok());
        // And huge parameters are refused before any memory is allocated.
        let mut json = read_json(&test_v);
        json["kdf_params"]["m_kib"] = u64::from(u32::MAX).into();
        write_json(&test_v, &json);
        assert!(matches!(
            prod.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
        assert!(matches!(
            test_v.unlock_with_password(&secret(PW)),
            Err(VaultError::UnlockFailed)
        ));
    }

    #[test]
    fn calibration_math() {
        let target = Duration::from_millis(500);
        // Slow computer: stay at the floor.
        assert_eq!(
            calibrated_params(Duration::from_millis(900), target),
            KDF_FLOOR
        );
        assert_eq!(calibrated_params(target, target), KDF_FLOOR);
        // Twice as fast as needed: double the memory.
        let p = calibrated_params(Duration::from_millis(250), target);
        assert_eq!(
            p,
            KdfParams {
                m_kib: 128 * 1024,
                t: 3,
                p: 1
            }
        );
        // Very fast: memory hits the ceiling (1 GiB, a phone 256 MiB), then passes grow (up to 10).
        let p = calibrated_params(Duration::from_millis(10), target);
        assert_eq!(p.m_kib, CALIBRATION_MAX_M_KIB);
        assert_eq!(p.t, if cfg!(target_os = "android") { 10 } else { 9 });
        let p = calibrated_params(Duration::from_nanos(1), target);
        assert_eq!(
            p,
            KdfParams {
                m_kib: CALIBRATION_MAX_M_KIB,
                t: 10,
                p: 1
            }
        );
        // Whatever comes out is always accepted by the reader.
        for ms in [1, 5, 50, 120, 333, 499, 501, 5_000] {
            let p = calibrated_params(Duration::from_millis(ms), target);
            assert!(KdfPolicy::Production.accepts(&p), "{ms} ms → {p:?}");
        }
    }

    /// One real setup with production calibration (Argon2 is built optimised even
    /// in test builds, see the workspace Cargo.toml). Proves calibrated
    /// parameters are inside the accepted range and round-trip through the file.
    #[test]
    fn production_calibration_round_trip() {
        let dir = TestDir::new("prod");
        let v = VaultDir::new(dir.path());
        let started = Instant::now();
        let (vault, key) = v.create(&secret(PW)).unwrap();
        let setup_took = started.elapsed();
        let params: KdfParams =
            serde_json::from_value(read_json(&v)["kdf_params"].clone()).unwrap();
        assert!(KdfPolicy::Production.accepts(&params), "{params:?}");
        assert!(params.m_kib >= KDF_FLOOR.m_kib && params.t >= KDF_FLOOR.t);
        let again = v.unlock_with_password(&secret(PW)).unwrap();
        assert_eq!(again.dek().expose(), vault.dek().expose());
        assert!(v.unlock_with_recovery_key(&key.display()).is_ok());
        eprintln!("production setup took {setup_took:?} with {params:?}");
    }

    #[test]
    fn nonces_and_salts_are_fresh_each_time() {
        let (_d, v, _vault, _key) = setup("fresh");
        let first = read_json(&v);
        v.change_password(&secret(PW), &secret(PW)).unwrap();
        let second = read_json(&v);
        assert_ne!(first["nonces"]["password"], second["nonces"]["password"]);
        assert_ne!(first["salts"]["password"], second["salts"]["password"]);
        assert_ne!(
            first["wrapped_dek_password"],
            second["wrapped_dek_password"]
        );
    }

    #[test]
    fn a_pin_wrap_unlocks_with_its_pin_only_and_stays_out_of_the_file() {
        let (_t, v, vault, _key) = setup("pin");
        let before = std::fs::read(v.vault_path()).unwrap();
        let wrap = v.wrap_for_pin(&vault, &secret("042917")).unwrap();
        let opened = wrap.unlock(&secret("042917")).unwrap();
        assert_eq!(opened.dek().expose(), vault.dek().expose());
        assert!(matches!(
            wrap.unlock(&secret("042918")),
            Err(VaultError::UnlockFailed)
        ));
        assert_eq!(std::fs::read(v.vault_path()).unwrap(), before);
        let again = v.wrap_for_pin(&vault, &secret("042917")).unwrap();
        assert_ne!(*again.salt, *wrap.salt);
        assert_ne!(again.nonce, wrap.nonce);
    }
}
