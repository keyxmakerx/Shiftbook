# Security

## Threat model (Phase 1, desktop)
Single user on their own or a work laptop. Protect against: data leaking off the machine, malicious or
compromised dependencies, the app being reachable from the network, accidental data loss, and
work data ending up in Git.

## Requirements
1. **No listening sockets.** The app never opens a port. (Tauri IPC only.)
2. **No outbound network.** No remote scripts, fonts, images, analytics, telemetry or update checks.
3. **Content Security Policy** in `tauri.conf.json`:
   `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:;
   font-src 'self'; connect-src ipc: http://ipc.localhost; object-src 'none';
   base-uri 'none'; frame-ancestors 'none'; form-action 'none'`
   (no `unsafe-eval`; avoid `unsafe-inline`).
4. **Tauri capabilities:** allow-list only the commands and plugin permissions actually used.
   `fs` scoped to the app data dir and user-chosen files via the dialog plugin. No `shell`.
5. **Input handling:** all SQL via sqlx parameter binding. Never build SQL with string formatting.
   Render user text as text (Svelte default escaping); no `{@html}` with user content.
6. **Dependencies:** minimal; ask before adding. Commit lockfiles. Enable Dependabot (cargo + npm +
   GitHub Actions). CI runs `cargo audit` and `npm audit --omit=dev` (fail on high/critical).
7. **Git hygiene:** `.gitignore` blocks databases, exports, backups and `.env`. Branch protection on `main`.
   The repo holds code and docs only, never anyone's work data or employer details.
8. **Backups:** automatic daily encrypted local backups, Backup now and Restore (see "Backup file
   format"), plus manual Export later. Restores are tested by `cargo test`; a CI restore of a
   fixture backup is still to do.
9. **Encryption at rest is mandatory (from milestone M1)** — see "Encryption design" below.
   There is no unencrypted mode. Readable files only exist when the user explicitly exports.
10. **Releases:** built only by GitHub Actions from tagged commits. Code signing (Windows
    Authenticode / Apple notarisation) before distributing beyond the author; until then, expect
    SmartScreen/Gatekeeper warnings and check with IT before running unsigned builds.

## Encryption design (password-manager style)
- **Key hierarchy**
  - A random 256-bit **Data Encryption Key (DEK)** is generated at first-run setup (OS CSPRNG).
  - **Password KEK** = Argon2id(master password, random 16-byte salt). Starting parameters:
    m = 64 MiB, t = 3, p = 1; tune so unlock takes ~0.5 s on a normal laptop. Store params with the salt.
  - **Recovery KEK** = Argon2id(recovery key, separate salt). The recovery key is 256 random bits
    shown once as words/code; it is never stored.
  - The DEK is stored only **wrapped** (XChaCha20-Poly1305) by each KEK in `vault.json` next to the
    database: `{version, kdf, kdf_params, salts, nonces, wrapped_dek_password, wrapped_dek_recovery}`.
  - Change password = re-wrap the DEK only (no full re-encryption). Regenerate recovery key = new
    recovery wrap; the old one stops working.
- **Database:** SQLCipher (via `libsqlite3-sys` `bundled-sqlcipher-vendored-openssl` with sqlx),
  keyed with the raw DEK (`PRAGMA key = "x'…'"`). Verify on all three OSes in CI. If SQLCipher proves
  unworkable on a platform, stop and ask before choosing an alternative.
- **In memory:** keep keys in `zeroize`/`secrecy` types; on lock or auto-lock, close the DB
  connection pool and zeroize the DEK. The frontend never receives keys or the password after
  submission; unlock is a single Tauri command.
- **Unlock attempts:** exponential back-off after 5 failures; generic error message.
- **Crypto rules:** use audited RustCrypto crates only (`argon2`, `chacha20poly1305`, `rand`/`getrandom`,
  `zeroize`). No home-made crypto, no custom ciphers, unique random nonce per encryption.

## Backup file format (`.shiftbak`)
```
magic "SBAK" | u16 BE format_version (1) | u32 BE header_len | header (JSON) | ciphertext
header: { app_version, created_at, schema_version, cipher: "xchacha20poly1305",
          compression: "gzip", nonce, key_check, vault }
ciphertext: XChaCha20-Poly1305( DEK, nonce, gzip( JSON of every table, schema_version N ) )
            with every byte before it (magic, version, length, header) as AAD
```
- **Same key as the data.** The payload is encrypted with the vault's DEK, which never changes
  (a password change only re-wraps it). So a backup made on this computer opens with the
  current session, with no password asked again.
- **Self-contained.** `vault` is a copy of `vault.json` (the DEK wrapped by the password and the
  recovery key, never the DEK itself) as it was when the backup was made. On a new computer, or
  after Delete all data made a new vault, the backup opens with the password **or** recovery key
  from that time. The KDF parameters in that copy are clamped like `vault.json`'s.
- **`key_check`** is the Poly1305 tag of an empty message under the DEK, with an all-zero nonce
  and the label `shiftbook-backup-key-check-v1` as AAD. It only tells "made with another key"
  (ask for the secret) apart from "damaged". It reveals nothing about the DEK.
- **Tamper-evident.** The header is bound as AAD, so any changed byte is refused before anything
  is replaced. Header size (64 KiB), file size (512 MiB) and unpacked size (2 GiB) are capped;
  unknown header fields and a newer `schema_version` are refused.
- **Restore flow:** open and verify → preview counts → write a safety backup of the current data
  (`backups/before-restore-*.shiftbak`) → replace every table in one transaction (columns are
  checked against the current schema) → reload the UI.
- **Automatic backups** use the same format: `backups/auto-YYYYMMDD-HHMMSS.shiftbak` in the app
  data folder (files `0600`), made in the background at most once a day after unlock, keeping the
  last 14. Safety copies are not pruned. **Delete all data keeps the backups folder**; those
  backups still need the old password or recovery key.
- **Dialogs run in Rust.** Backup now and Restore from a file use the dialog plugin from the
  commands; the WebView has no dialog or file-system permission and refers to a picked file only
  as `"picked"`. The frontend never sees a path, a key or file bytes.

## Exports (unencrypted by design)
- Excel (.xlsx via `rust_xlsxwriter`), CSV (UTF-8 with BOM, via `csv` crate; optional zip), JSON.
- Always behind an explicit user action with a warning that the file is not protected.
- Never written automatically, never placed in the app's data/backup folders.
- CSV cells starting with `=`, `+`, `-`, `@` are prefixed with `'` to prevent formula injection
  when opened in Excel.

## Phase 2 additions (hosted)
Desktop encryption stays as-is. The server uses OIDC SSO, HTTPS only (behind reverse proxy), secure session cookies, CSRF protection for
state-changing requests, per-owner authorization checks on every query, rate limiting, security
headers (HSTS, CSP, X-Content-Type-Options, Referrer-Policy), audit log for admin actions,
encrypted database at rest, secrets from env/secret store only, non-root container, image scanning.

## Using it at work
Shiftbook keeps everything on the user's machine. Whether work notes may be stored in a local
app is the user's employer's rule to set; the app does nothing that needs a network or a server.

## Build-time code
Node and npm run only on the developer's machine and in CI, to compile `web/` into static files.
The shipped app has no Node runtime. `npm audit` still runs in CI because the compiled bundle
contains code from those packages.

## M1 security checklist (research, Oct 2026)
Gathered before building the vault; sources are the projects' own advisories and docs. Items marked
*unverified* were not confirmed against a primary source and get a test instead.

**Advisories checked against locked versions:** tauri 2.12.4 is past GHSA-w28w-mhc8-qvjv (channel
fetch ACL bypass, fixed 2.11.6) and CVE-2026-42184 (`is_local_url` remote IPC, fixed 2.11.1).
`tauri-plugin-fs` must be ≥ 2.5.2 when added (GHSA-9g54-6x48-9vpw). Svelte 5.57.2 is past the
client-side DOM-clobbering advisory GHSA-rcqx-6q8c-2c42 (fixed 5.55.7); still never spread user data
onto elements. Vite 8.3.4 is past the Oct 2026 dev-server file-read advisories (fixed 8.3.3); never
run the dev server with `--host`. SQLCipher: take the newest `libsqlite3-sys` sqlx accepts and check
the bundled SQLite/OpenSSL in `Cargo.lock`; never enable extension loading. WebView2 and WebKitGTK
are updated by the OS, not shipped by us; users should keep their OS updated.

**Must (M1):**
1. DEK, salts and nonces from OS randomness (`OsRng` / `getrandom`).
2. XChaCha20-Poly1305 with a fresh random 24-byte nonce for every wrap and every backup; bind
   version, KDF params, salt and purpose (`pw` / `recovery`) as AAD.
3. Argon2id calibrated at first run to ~0.5 s, floor m = 64 MiB, t = 3, p = 1, cap ~1 GiB. KDF
   params read from `vault.json` or a `.shiftbak` are untrusted: clamp to a range and refuse below
   the floor. The AEAD tag check is the only password check (no separate hash compare).
4. Password, recovery key and keys only in `secrecy`/`zeroize` types; no `Debug`/`Serialize` on
   anything holding secrets; `argon2`'s `zeroize` feature on; errors cross IPC as generic codes.
5. Vault writes are atomic: temp file in the same folder, fsync, rename over, fsync the folder on
   Unix; keep `vault.json.prev`. Change password re-derives from the current password first and
   writes the new vault before dropping the old wrap. Regenerating the recovery key needs the password.
6. SQLCipher: `PRAGMA key = "x'…'"` as the first statement on every connection,
   `cipher_compatibility = 4`, no `cipher_plaintext_header_size`, never `temp_store = FILE`,
   sqlx statement logging off. sqlx keeps an un-zeroized copy of the hex key; documented limit.
7. Lock: close the pool, zeroize the DEK, reload the WebView so decrypted state is gone. Idle
   auto-lock and lock on OS sleep/lock.
8. Unlock is one command; the password never returns to JS; one generic "Couldn't unlock" message;
   back-off is kept in memory (it is UX only; Argon2 cost is the real defence); never wipe data
   after failed attempts (deleting everything is only ever the user's own typed choice).
9. Recovery key shown once with a retype check of a few words; no automatic copy; if copied, warn
   that clipboard history/sync may keep it, and clear the clipboard after 60 s.
10. CI on all three OSes proves the database cannot be opened without the key, and opens after a
    password change and with the recovery key.

**Should:** `cipher_memory_security = ON`; `freezePrototype: true` (done); files 0600 / folders
0700 on Unix; `autocomplete="off"` on secret fields; bound `.shiftbak` header length and
decompressed size, and JSON size/depth, before restore/import lands; test sqlx pragma order.

**Later:** Tauri isolation pattern, `mlock` for the key page, `cargo-vet`, disabling WebView2
autofill, clearing the clipboard on lock.

**Known limits (tell the user plainly):** the password briefly exists in the WebView's memory and
IPC buffers; swap, hibernation files and crash dumps may hold memory; screenshot protection works on
Windows, is partial on macOS and does nothing on Linux; nothing stops a phone camera.

## M1 limits
What the M1 build does not protect against, or doesn't do yet:
- **sqlx keeps a copy of the key.** The `PRAGMA key` text (with the DEK in hex) lives in sqlx's
  connect options as an ordinary `String` for as long as the database is open, and is not wiped
  on lock (only dropped). Our own copies are wiped.
- **The password passes through the WebView.** It is typed in the page, sent over Tauri IPC as a
  string and only then moved into a wiped buffer. JavaScript strings can't be wiped; the form
  fields are cleared on submit and the page is reloaded on lock. The recovery key is sent back to
  the page once at setup.
- **OS sleep and screen lock aren't detected directly.** The page locks when its timer notices a
  gap of a minute or more (the computer slept or hibernated). Locking the screen without sleeping
  only triggers the 15-minute idle lock.
- **Old copies of `vault.json`** (for example in a user's own file backup) still open the database
  with the password that was current then, because the DEK never changes. `vault.json.prev` is
  kept equal to the current vault for this reason.
- **The unlock back-off is in memory only** and resets when the app restarts. Argon2's cost is the
  real defence against guessing.
- **After a recovery-key unlock** the app asks for a new password at once (alpha). The new
  password wraps the key already unlocked in memory; the command is refused unless the session
  was just unlocked with the recovery key, and works once.
- **Recovery key confirmation:** if the app is closed on the recovery key screen, the key is gone;
  make a new one in Settings → Security (needs the password; the old key stops working).
- **WebView cache files** (WebKitGTK on Linux) sit in the app data folder with default
  permissions. Shiftbook puts no data in WebView storage.
- **SQLCipher errors are printed to the terminal** (for example "hmac check failed" on a wrong
  key). They contain page numbers, never keys.
- The general known limits above (swap, hibernation, crash dumps, screenshots) still apply.

**Bundled database versions (M1):** sqlx 0.9 caps `libsqlite3-sys` below 0.38, which bundles
SQLCipher 4.10.0 on SQLite 3.50.4 and OpenSSL 3.6.3. SQLCipher 4.15/4.17 fix CVE-2026-51936
(`sqlcipher_export` in defensive mode) and FTS5 issues (CVE-2026-11822/-11824); Shiftbook uses
neither feature. Upgrade as soon as sqlx accepts a newer `libsqlite3-sys`.

## Alpha additions
- **Screenshot and copy protection:** the window starts content-protected. The Protected pill
  calls `allow_capture`, which turns protection off and starts a 10-minute timer inside the app
  (not the page) that turns it back on; `protect_capture` and `lock` turn it on at once. While
  protected the page blocks `copy` and `cut` everywhere (text boxes included); the explicit Copy
  buttons write through the Clipboard API and still work. Copying the recovery key keeps its
  60-second clipboard clear.
- **Links are never opened.** Link fields (for example Jira) only build text to show and copy;
  templates must be `http(s)://…{ticket}…` and the ticket is percent-encoded. No opener plugin.
- **New commands** each have their own permission; errors stay short codes. The capability still
  grants no plugins.
- **Delete all data** (`delete_all_data`, Settings → Security and the unlock screen's recovery-key
  mode): needs the typed word `DELETE`, checked in `shiftbook_core::wipe` (the page only uses it to
  enable the button). It needs no password, so it works for someone who lost both the password
  and the recovery key; anyone at the computer could delete the files by hand anyway. The command
  closes the database, then deletes `shiftbook.db` and its side files, leftover vault temp files,
  `vault.json.prev` and last `vault.json` (so a failure part-way still leaves the unlock screen).
  Files are unlinked, not overwritten: SSDs make overwriting unreliable, and once the vault is
  gone the database key is gone, so recovered database blocks can't be read. An old copy of
  `vault.json` in a user's own file backup, together with recovered database blocks and the
  password of that time, would still open them (see "Old copies of `vault.json`" above).

## Quick unlock with a PIN
Approved in sign-off round 15 and built then. Rules: `crates/core/src/quickunlock.rs`; the
wrap: `PinWrap` in `crates/storage/src/vault.rs`; the window: `PinWindow` in
`apps/desktop/src-tauri/src/session.rs`.

**The problem.** Auto-lock after 15 minutes idle is the right default, but every unlock needs the
full master password (12+ characters). On a day of short breaks people turn auto-lock off or
raise it to hours, which leaves the app open for longer than a PIN would.

**What it does.** An optional 6-digit PIN, turned on in Settings → Security (needs the password). After
an idle lock or Lock now, the unlock screen asks for the PIN instead of the password. It works
only inside a **PIN window** that starts at a full unlock (password or recovery key) and ends at
the first of: midnight, 10 hours after that unlock, the app quitting, the computer sleeping (the
gap the page already detects), 3 wrong PINs, or "Use my password instead" on the unlock screen.
Outside the window the unlock screen asks for the password as now.

**Typing the PIN each day.** The PIN is never stored and the window ends at midnight, so the app
has to be told the PIN once a day. After the day's first password unlock, a small box asks for
today's PIN (Skip leaves locks on the password for the day); Settings → Security can set it too.
Turning Quick unlock on stores only a flag (`security.quick_unlock`), never the PIN.

**How the key is held during a PIN lock.** The database is closed and the DEK is wiped exactly as
in a normal lock. When today's PIN is typed (while unlocked), the DEK is wrapped
(XChaCha20-Poly1305, fresh random nonce, AAD purpose `pin`) with a **PIN KEK** = Argon2id(PIN,
random 16-byte salt, same parameters as the password KEK), and the PIN and KEK are wiped at once;
the wrap stays for the window. The
wrapped DEK, salt and nonce are kept **in memory only**, in `zeroize` types in the Rust session.
They are never written to disk, to `vault.json` or to backups. A PIN unlock derives the PIN KEK,
unwraps the DEK and opens the database as a normal unlock does.

**Wrong PINs.** The attempt count lives in the Rust session, not the page, and counts wrong PINs
in a row (a right one starts it again). The third wrong PIN wipes the wrapped DEK and the salt at
once, so further guesses are impossible and only the
password or recovery key can open the app. There is no back-off delay for PINs; the hard limit of
3 replaces it.

**What a PIN protects, and what it doesn't.**
- The database file on disk is unchanged: encrypted with the DEK, which is wrapped only by the
  password and recovery KEKs. Copying the file (or a backup) and guessing the PIN gets nothing.
- Someone at the unlocked computer during a PIN lock gets 3 guesses at 1,000,000 PINs.
- Someone who can read Shiftbook's memory during a PIN lock (malware running as the user, a
  memory dump, a hibernation file) can find the wrapped DEK and try all PINs offline. At ~0.5 s
  per Argon2id guess that is roughly 6 core-days for the whole space, much less on many cores.
  Today the same attacker reading memory while the app is **unlocked** gets the DEK directly, so
  this adds a small window, not a new kind of attack. It is why the PIN window ends at sleep
  (hibernation) and after 10 hours.
- The PIN itself is never stored. Only the in-memory salt and wrapped DEK exist, and only during
  the window.

**Not chosen.**
- Keeping the database open behind a lock screen ("soft lock"): simpler, but the DEK and sqlx's
  copy of the key would stay in memory in the clear for the whole lock.
- Storing a PIN-wrapped key on disk so the PIN survives a restart: a 6-digit PIN on disk is
  brute-forced offline in days, which would make the PIN the real strength of the file.
- The OS keychain or Windows Hello: needs new plugins and behaves differently on each OS; the
  Android thread's fingerprint option can revisit this.

**Sleep.** Two checks end the window after the computer sleeps: the page's own timer gap (as for
auto-lock, on the unlock screen too), and in Rust the computer's clock running more than 60 s
ahead of the app's monotonic clock since the full unlock (the monotonic clock stops during sleep
on Linux and macOS).

**Tests** (`quick_unlock_with_a_pin` in session.rs, plus core and vault tests). PIN unlock inside the window; refusal after midnight, after 10 hours,
after quit and after a sleep gap; the third wrong PIN wipes the wrapped key (a fourth attempt with
the right PIN fails); nothing PIN-related appears in `vault.json`, the database or a backup;
changing the password or recovery key ends the window.
