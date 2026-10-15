// Phase 1 implementation of DataApi. This is the ONLY file in web/ allowed to import
// from @tauri-apps/* (CLAUDE.md rule 1).
import { invoke } from '@tauri-apps/api/core';
import type { AppInfo, DataApi } from './DataApi';

export class TauriApi implements DataApi {
  appInfo(): Promise<AppInfo> {
    return invoke<AppInfo>('app_info');
  }
}
