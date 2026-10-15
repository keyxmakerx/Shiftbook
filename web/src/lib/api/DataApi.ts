// The single data contract between the UI and the app. Every method maps 1:1 to a Tauri
// command (Phase 1, `TauriApi`) and to a REST endpoint (Phase 2, `HttpApi`).
// Components import `api` from `$lib/api` (this folder's index) and never talk to Tauri directly.

/** Mirrors `AppInfo` in apps/desktop/src-tauri/src/lib.rs. */
export interface AppInfo {
  name: string;
  version: string;
}

export interface DataApi {
  /** App name and version. Tauri: `app_info`. Phase 2: `GET /api/app-info`. */
  appInfo(): Promise<AppInfo>;
}
