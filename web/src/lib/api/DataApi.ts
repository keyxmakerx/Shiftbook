// The single data contract between the UI and the app. Every method maps 1:1 to a Tauri
// command (Phase 1, `TauriApi`) and to a REST endpoint (Phase 2, `HttpApi`).
// Components import `api` from `$lib/api` (this folder's index) and never talk to Tauri directly.

/** Mirrors `AppInfo` in apps/desktop/src-tauri/src/lib.rs. */
export interface AppInfo {
  name: string;
  version: string;
}

/** Mirrors `VaultStatus` in apps/desktop/src-tauri/src/session.rs. */
export type VaultStatus = 'needs_setup' | 'locked' | 'unlocked';

/** Which secret `unlock` is given. */
export type SecretKind = 'password' | 'recovery_key';

/** Mirrors `CommandError` in apps/desktop/src-tauri/src/session.rs, plus `unknown`. */
export type ApiErrorCode =
  | 'unlock_failed'
  | 'weak_password'
  | 'busy'
  | 'already_set_up'
  | 'not_set_up'
  | 'locked'
  | 'storage_error'
  | 'unknown';

const KNOWN_CODES: readonly ApiErrorCode[] = [
  'unlock_failed',
  'weak_password',
  'busy',
  'already_set_up',
  'not_set_up',
  'locked',
  'storage_error',
];

/** Every DataApi failure is one of these: a short code, never details or secrets. */
export class ApiError extends Error {
  readonly code: ApiErrorCode;

  constructor(code: ApiErrorCode) {
    super(code);
    this.name = 'ApiError';
    this.code = code;
  }
}

/** Turn whatever an adapter rejected with into an ApiError with a known code. */
export function toApiError(raw: unknown): ApiError {
  if (raw instanceof ApiError) return raw;
  if (typeof raw === 'string' && (KNOWN_CODES as readonly string[]).includes(raw)) {
    return new ApiError(raw as ApiErrorCode);
  }
  return new ApiError('unknown');
}

export interface DataApi {
  /** App name and version. Tauri: `app_info`. Phase 2: `GET /api/app-info`. */
  appInfo(): Promise<AppInfo>;

  /** First run, locked or unlocked. Tauri: `vault_status`. */
  vaultStatus(): Promise<VaultStatus>;

  /**
   * First-run setup with a new master password (12+ characters). Resolves with the recovery
   * key, shown to the user once and never again. Tauri: `vault_setup`.
   */
  vaultSetup(password: string): Promise<string>;

  /** The user confirmed they saved the recovery key. Tauri: `vault_confirm_recovery_saved`. */
  vaultConfirmRecoverySaved(): Promise<void>;

  /** Unlock with the password or the recovery key. Tauri: `unlock`. */
  unlock(secret: string, kind: SecretKind): Promise<void>;

  /** Close the data file and forget the key. Reload the page afterwards. Tauri: `lock`. */
  lock(): Promise<void>;

  /** Change the master password; needs the current one. Tauri: `change_password`. */
  changePassword(currentPassword: string, newPassword: string): Promise<void>;
}
