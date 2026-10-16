// The single data contract between the UI and the app. Every method maps 1:1 to a Tauri
// command (Phase 1, `TauriApi`) and to a REST endpoint (Phase 2, `HttpApi`).
// Components import `api` from `$lib/api` (this folder's index) and never talk to Tauri directly.
// Shapes mirror the views in apps/desktop/src-tauri/src/api.rs and the serialized core types.

/** Mirrors `AppInfo` in apps/desktop/src-tauri/src/lib.rs. */
export interface AppInfo {
  name: string;
  version: string;
}

/** Mirrors `VaultStatus` in apps/desktop/src-tauri/src/session.rs. */
export type VaultStatus = 'needs_setup' | 'locked' | 'unlocked';

/** Which secret `unlock` is given. */
export type SecretKind = 'password' | 'recovery_key';

/** What `unlock` tells the UI. */
export interface UnlockOutcome {
  /** Unlocked with the recovery key: ask for a new password straight away. */
  mustSetPassword: boolean;
}

/** ISO local date, `YYYY-MM-DD`. */
export type IsoDate = string;

export type Theme = 'system' | 'light' | 'dark';
export type Motion = 'still' | 'calm' | 'lively';
export type GroupBy = 'code' | 'project';
export type WeekStartDay = 'monday' | 'tuesday' | 'wednesday' | 'thursday' | 'friday' | 'saturday' | 'sunday';

export interface CommentFormat {
  includeTickets: boolean;
  separator: string;
}

/** Mirrors `shiftbook_core::prefs::Prefs`. */
export interface Prefs {
  theme: Theme;
  motion: Motion;
  accent: string | null;
  groupBy: GroupBy;
  comment: CommentFormat;
  weekStart: WeekStartDay;
  autoLockMinutes: number;
  /** Daily target in minutes, Monday first (seven numbers). */
  dayTargets: number[];
  /** Text size in percent: 90, 100, 115 or 130. */
  textSize: number;
  /** End-of-day wrap-up time `HH:MM`, or null when it's off. */
  wrapupAt: string | null;
}

/** Setting keys `savePref` accepts (see `shiftbook_core::prefs`). */
export type PrefKey =
  | 'ui.theme'
  | 'ui.motion'
  | 'ui.accent'
  | 'timesheet.group_by'
  | 'timesheet.comment_tickets'
  | 'timesheet.comment_separator'
  | 'calendar.week_start'
  | 'security.auto_lock_minutes'
  | 'day.targets'
  | 'ui.text_size'
  | 'day.wrapup_at';

export interface Project {
  id: string;
  name: string;
  color: string | null;
  defaultTimesheetCodeId: string | null;
  archived: boolean;
}

export interface ProjectInput {
  name: string;
  color: string | null;
  defaultTimesheetCodeId: string | null;
  archived: boolean;
}

export interface TicketLink {
  fieldName: string;
  url: string;
}

export interface Ticket {
  id: string;
  number: string;
  links: TicketLink[];
}

export interface Entry {
  id: string;
  date: IsoDate;
  minutes: number;
  hours: string;
  description: string;
  projectId: string | null;
  category: string | null;
  notes: string | null;
  timesheetCodeId: string | null;
  tickets: Ticket[];
  /** The day's roll-up of the Done tickets tray. */
  isTray: boolean;
}

export interface EntryInput {
  date: IsoDate;
  minutes: number;
  description: string;
  projectId: string | null;
  category: string | null;
  notes: string | null;
  timesheetCodeId: string | null;
}

export interface TrayItem {
  id: string;
  ticket: string;
  minutes: number;
  links: TicketLink[];
}

export interface TrayDay {
  date: IsoDate;
  items: TrayItem[];
  entryId: string | null;
  minutes: number;
  hours: string;
}

/** Mirrors `shiftbook_core::capture::Capture`. */
export interface Capture {
  minutes: number | null;
  hoursText: string | null;
  tickets: string[];
  projectId: string | null;
  projectQuery: string | null;
  date: IsoDate;
  dayWord: string | null;
  description: string;
}

export interface DayPart {
  projectId: string | null;
  minutes: number;
}

export interface RibbonDay {
  date: IsoDate;
  minutes: number;
  hours: string;
  parts: DayPart[];
  future: boolean;
  /** Nothing logged on a day that has a target. */
  gap: boolean;
}

export interface RibbonWeek {
  weekStart: IsoDate;
  minutes: number;
  hours: string;
  days: RibbonDay[];
}

export interface SheetItem {
  title: string;
  tickets: string[];
  entryIds: string[];
  minutes: number;
}

export interface SheetLine {
  key: string;
  area: string;
  label: string;
  minutes: number;
  hours: string;
  comment: string;
  items: SheetItem[];
  unassigned: boolean;
  projectId: string | null;
}

export interface SheetDay {
  date: IsoDate;
  minutes: number;
  hours: string;
  lines: SheetLine[];
}

export interface ProjectEffort {
  projectId: string | null;
  name: string;
  minutes: number;
  hours: string;
}

export type SheetWarning =
  | { kind: 'unassigned'; minutes: number; hours: string }
  | { kind: 'nothing_logged'; date: IsoDate }
  | { kind: 'long_day'; date: IsoDate; hours: string };

/** Mirrors `shiftbook_core::sheet::WeekSheet`. */
export interface WeekSheet {
  weekStart: IsoDate;
  groupBy: GroupBy;
  days: SheetDay[];
  effort: ProjectEffort[];
  minutes: number;
  hours: string;
  ticketCount: number;
  entryCount: number;
  warnings: SheetWarning[];
  tsv: string;
}

export interface Tick {
  day: IsoDate;
  lineKey: string;
  done: boolean;
}

export interface TimesheetCode {
  id: string;
  area: string;
  phaseLabel: string;
  code: string;
  sortOrder: number;
  hidden: boolean;
}

export interface CodeInput {
  area: string;
  phaseLabel: string;
  code: string;
  sortOrder: number;
  hidden: boolean;
}

export interface CategoryCode {
  category: string;
  codeId: string | null;
}

export type FieldKind = 'text' | 'number' | 'choice' | 'link';
export type AppliesTo = 'entry' | 'ticket' | 'project';

export interface CustomField {
  id: string;
  appliesTo: AppliesTo;
  name: string;
  kind: FieldKind;
  options: string[];
  linkTemplate: string | null;
  ticketPrefix: string | null;
  hidden: boolean;
}

export interface FieldInput {
  appliesTo: AppliesTo;
  name: string;
  kind: FieldKind;
  /** Choices, one per line. */
  options: string;
  linkTemplate: string;
  ticketPrefix: string;
  hidden: boolean;
}

export interface CustomValue {
  fieldId: string;
  value: string;
  /** For link fields: the value turned into a link (shown and copied, never opened). */
  link: string | null;
}

export interface InboxItem {
  id: string;
  text: string;
  createdAt: string;
}

/** One thing on the "next up" list (written at the wrap-up, shown by "Where was I?"). */
export interface NextUpItem {
  id: string;
  text: string;
  done: boolean;
}

/** What Find turned up, best first. */
export interface SearchResult {
  entries: Entry[];
  inbox: InboxItem[];
  /** Indexes into the command names passed to `search`. */
  commands: number[];
}

/** Mirrors `CommandError` in apps/desktop/src-tauri/src/session.rs, plus `unknown`. */
export type ApiErrorCode =
  | 'unlock_failed'
  | 'weak_password'
  | 'busy'
  | 'already_set_up'
  | 'not_set_up'
  | 'locked'
  | 'storage_error'
  | 'invalid_input'
  | 'not_found'
  | 'duplicate'
  | 'tray_managed'
  | 'not_allowed'
  | 'unknown';

const KNOWN_CODES: readonly ApiErrorCode[] = [
  'unlock_failed',
  'weak_password',
  'busy',
  'already_set_up',
  'not_set_up',
  'locked',
  'storage_error',
  'invalid_input',
  'not_found',
  'duplicate',
  'tray_managed',
  'not_allowed',
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
  // ----- app and vault -----
  /** App name and version. Tauri: `app_info`. Phase 2: `GET /api/app-info`. */
  appInfo(): Promise<AppInfo>;
  /** First run, locked or unlocked. Tauri: `vault_status`. */
  vaultStatus(): Promise<VaultStatus>;
  /** First-run setup; resolves with the recovery key, shown once. Tauri: `vault_setup`. */
  vaultSetup(password: string): Promise<string>;
  /** The user confirmed they saved the recovery key. Tauri: `vault_confirm_recovery_saved`. */
  vaultConfirmRecoverySaved(): Promise<void>;
  /** Unlock with the password or the recovery key. Tauri: `unlock`. */
  unlock(secret: string, kind: SecretKind): Promise<UnlockOutcome>;
  /** Close the data file and forget the key. Reload the page afterwards. Tauri: `lock`. */
  lock(): Promise<void>;
  /** Change the master password; needs the current one. Tauri: `change_password`. */
  changePassword(currentPassword: string, newPassword: string): Promise<void>;
  /** New password straight after a recovery-key unlock. Tauri: `set_new_password`. */
  setNewPassword(newPassword: string): Promise<void>;
  /** Make a new recovery key (needs the password); resolves with it, shown once. */
  regenerateRecoveryKey(password: string): Promise<string>;

  // ----- screenshot protection -----
  /** Allow screenshots/screen sharing for a while; resolves with the seconds. */
  allowCapture(): Promise<number>;
  /** Protect again now. */
  protectCapture(): Promise<void>;

  // ----- preferences -----
  loadPrefs(): Promise<Prefs>;
  /** Save one preference; resolves with all preferences after the change. */
  savePref(key: PrefKey, value: string): Promise<Prefs>;

  // ----- projects -----
  listProjects(): Promise<Project[]>;
  createProject(name: string, color: string | null): Promise<Project>;
  updateProject(id: string, input: ProjectInput): Promise<Project>;

  // ----- entries and tickets -----
  /** Live entries from `from` to `to`, inclusive. */
  listEntries(from: IsoDate, to: IsoDate): Promise<Entry[]>;
  /** `tickets` is pasted text; it is split like multi-paste. */
  createEntry(input: EntryInput, tickets: string): Promise<Entry>;
  updateEntry(id: string, input: EntryInput): Promise<Entry>;
  /** Add (or with a negative number take away) minutes. */
  addEntryMinutes(id: string, delta: number): Promise<Entry>;
  deleteEntry(id: string): Promise<void>;
  restoreEntry(id: string): Promise<Entry>;
  addEntryTickets(id: string, text: string): Promise<Entry>;
  removeTicket(ticketId: string): Promise<Entry>;
  /** Log work: tickets or words plus a time button, on `date`. */
  quickLog(date: IsoDate, text: string, minutes: number): Promise<Entry>;

  // ----- capture -----
  /** What a capture line means (for the live chips). */
  parseCapture(text: string, today: IsoDate): Promise<Capture>;
  /** Log a capture line; it must contain hours. */
  captureLog(text: string, today: IsoDate): Promise<Entry>;

  // ----- Done tickets tray -----
  trayDay(date: IsoDate): Promise<TrayDay>;
  trayAdd(date: IsoDate, text: string): Promise<TrayDay>;
  traySetMinutes(id: string, minutes: number): Promise<TrayDay>;
  trayRemove(id: string): Promise<TrayDay>;
  trayRestore(id: string): Promise<TrayDay>;

  // ----- ribbon and timesheet -----
  /** The last four weeks up to the week of `today`. */
  ribbon(today: IsoDate): Promise<RibbonWeek[]>;
  /** The crib sheet for the week containing `date`. */
  weekSheet(date: IsoDate, today: IsoDate, groupBy: GroupBy | null): Promise<WeekSheet>;
  listTicks(weekStart: IsoDate): Promise<Tick[]>;
  setTick(weekStart: IsoDate, day: IsoDate, lineKey: string, done: boolean): Promise<void>;

  // ----- timesheet codes -----
  listTimesheetCodes(): Promise<TimesheetCode[]>;
  /** Adds when `id` is null, else updates. */
  saveTimesheetCode(id: string | null, input: CodeInput): Promise<TimesheetCode>;
  categoryCodes(): Promise<CategoryCode[]>;
  setCategoryCode(category: string, codeId: string | null): Promise<void>;

  // ----- custom fields -----
  listCustomFields(): Promise<CustomField[]>;
  saveCustomField(id: string | null, input: FieldInput): Promise<CustomField>;
  deleteCustomField(id: string): Promise<void>;
  listCustomValues(recordId: string): Promise<CustomValue[]>;
  /** Resolves with the value as stored (blank clears it). */
  setCustomValue(fieldId: string, recordId: string, value: string): Promise<string>;

  // ----- inbox -----
  listInbox(): Promise<InboxItem[]>;
  addInbox(text: string): Promise<InboxItem>;
  setInboxDeleted(id: string, deleted: boolean): Promise<void>;

  // ----- next up -----
  /** The "next up" list, in order. */
  nextUp(): Promise<NextUpItem[]>;
  /** Replace the list (blank lines dropped, at most 7); an empty list clears it. */
  setNextUp(items: string[]): Promise<NextUpItem[]>;
  tickNextUp(id: string, done: boolean): Promise<void>;

  // ----- find -----
  /** Entries, Inbox scraps and the given command names that match `query`. */
  search(query: string, today: IsoDate, commands: string[]): Promise<SearchResult>;
}
