import type { DataApi } from './DataApi';
import { TauriApi } from './TauriApi';

export type { AppInfo, DataApi } from './DataApi';

/** The active data API. Phase 2 will choose `HttpApi` here instead. */
export const api: DataApi = new TauriApi();
