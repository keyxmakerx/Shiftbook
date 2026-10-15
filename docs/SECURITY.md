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
