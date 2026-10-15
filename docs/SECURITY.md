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
8. **Backups:** automatic daily local backups + manual Export. Restores are tested in CI with a
   fixture database.
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
magic "PBAK" | u16 format_version | u32 header_len | header (JSON) | encrypted payload
header: { app_version, created_at, kdf, kdf_params, salt_password, salt_recovery,
          wrapped_dek_password, wrapped_dek_recovery, cipher: "xchacha20poly1305", nonce }
payload: XChaCha20-Poly1305( DEK, zstd/gzip( JSON export, schema_version N ) ), header as AAD
```
- Self-contained: restorable on any OS / new computer with the password **or** recovery key.
- Authenticated encryption (header bound as AAD) → tampering or corruption is detected before restore.
- Restore flow: verify → preview counts → safety backup of current data → replace in one transaction.
- Automatic backups use the same format.

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
   back-off is kept in memory (it is UX only; Argon2 cost is the real defence); never wipe data.
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
