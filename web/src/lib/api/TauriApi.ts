// Phase 1 implementation of DataApi. This is the ONLY file in web/ allowed to import
// from @tauri-apps/* (CLAUDE.md rule 1).
import { invoke } from '@tauri-apps/api/core';
import { toApiError } from './DataApi';
import type { AppInfo, DataApi, SecretKind, VaultStatus } from './DataApi';

/** Call a command; any rejection becomes an ApiError with a short code. */
async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw toApiError(raw);
  }
}

export class TauriApi implements DataApi {
  appInfo(): Promise<AppInfo> {
    return call<AppInfo>('app_info');
  }

  vaultStatus(): Promise<VaultStatus> {
    return call<VaultStatus>('vault_status');
  }

  vaultSetup(password: string): Promise<string> {
    return call<string>('vault_setup', { password });
  }

  vaultConfirmRecoverySaved(): Promise<void> {
    return call<void>('vault_confirm_recovery_saved');
  }

  unlock(secret: string, kind: SecretKind): Promise<void> {
    return call<void>('unlock', { secret, kind });
  }

  lock(): Promise<void> {
    return call<void>('lock');
  }

  changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return call<void>('change_password', { currentPassword, newPassword });
  }
}
