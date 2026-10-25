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

/** A file in the backups folder: made automatically, or the copy made just before a restore. */
export interface BackupFile {
  name: string;
  kind: 'automatic' | 'before_restore';
  /** UTC, RFC 3339. */
  createdAt: string;
  bytes: number;
}

/** What a backup holds, shown before restoring it. */
export interface BackupSummary {
  /** UTC, RFC 3339. */
  createdAt: string;
  entries: number;
  tickets: number;
  projects: number;
}

/** Where a backup to check or restore comes from: a name from `listBackups()`, or the picked file. */
export const PICKED_BACKUP = 'picked';

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
  /** The optional "Now doing" timer is turned on (off by default). */
  timer: boolean;
  /** A timer paused by locking, to pick up after unlock. */
  pausedTimer: PausedTimer | null;
  /** Days without a change before a Jot, Daily or Meeting note moves to the archive (0 = off). */
  notesAutoArchiveDays: number;
  /** Open windows and where they are. */
  windows: WindowSpot[];
  /** Where the Desk's "Done today" pile sits; null places it automatically. */
  donePile: [number, number] | null;
}

/** How a window shows. Mirrors `shiftbook_core::windows::WindowMode`. */
export type WindowMode = 'open' | 'rolled' | 'min' | 'full';

/** A notepad or note popped out into a window (round 12). */
export interface WindowSpot {
  kind: CardKind;
  id: string;
  mode: WindowMode;
  x: number;
  y: number;
  w: number;
  h: number;
}

/** A meeting that repeats on set weekdays at a set time; the bell reminds about it. */
export interface MeetingSeries {
  id: string;
  name: string;
  /** Weekdays it meets, Monday first. */
  days: boolean[];
  /** Start, minutes after local midnight. */
  at: number;
  /** Minutes ahead the bell starts reminding. */
  remindMinutes: number;
}

export type SeriesInput = Omit<MeetingSeries, 'id'>;

/** Where today's meeting stands. Mirrors `shiftbook_core::meetings::MeetingToday`. */
export type MeetingToday = { state: 'later' } | { state: 'soon'; minutes: number } | { state: 'started'; minutes: number };

/** One thing in the bell's panel. Mirrors `shiftbook_core::bell::BellItem`. */
export interface BellItem {
  when: 'now' | 'today';
  title: string;
  about:
    | { kind: 'meeting'; series: string; at: number; state: MeetingToday }
    | { kind: 'brought_up'; id: string; from: IsoDate }
    | { kind: 'chase'; card: string; on: IsoDate }
    | { kind: 'still_open'; tickets: string[] };
}

/** A chip on the bottom bar: an entry, a ticket or a note kept in view (round 14). */
export interface PinItem {
  kind: CardKind;
  id: string;
  title: string;
  /** The day to chase it, if any. */
  chaseOn: IsoDate | null;
  /** Its chase date has come: the chip is orange and the bell lists it. */
  due: boolean;
}

/** A timer paused by locking: the entry and what it had counted (never a start or end time). */
export interface PausedTimer {
  entryId: string;
  seconds: number;
  /** Quiet stretches noticed while it ran, in seconds. */
  quiet: number[];
}

/** What Stop offers to add, rounded to 15 minutes. */
export interface TimerOffer {
  minutes: number;
  /** Without the time away; only when that makes a difference. */
  withoutAway: number | null;
  awayMinutes: number;
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
  | 'day.wrapup_at'
  | 'day.timer'
  | 'timer.paused'
  | 'notes.auto_archive_days'
  | 'ui.windows'
  | 'desk.done_pile';

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
  /** Ticked done; false while still open (round 14). */
  done: boolean;
  /** Its notepad. */
  note: string | null;
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
  /** A roll-up of the Tickets tray (the day's plain one, or a project's). */
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
  /** The day it was worked on. */
  date: IsoDate;
  ticket: string;
  minutes: number;
  /** The ticket's notepad, if anything is written in it. */
  note: string | null;
  /** False while it is still open: worked on, not finished. */
  done: boolean;
  /** The project its minutes count under; null = the day's "Tickets" entry. */
  projectId: string | null;
  links: TicketLink[];
}

export interface TrayDay {
  date: IsoDate;
  items: TrayItem[];
  /** Still-open tickets from earlier days, back until they are ticked. */
  carried: TrayItem[];
  /** The plain "Tickets" roll-up entry (tickets without a project), if any. */
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
  /** For a thing brought up from a card: the day it comes up. */
  from: IsoDate | null;
}

/** A saved response: a message kept for pasting into tickets, copied from the tray. */
export interface SavedResponse {
  id: string;
  /** Name shown in the menu. */
  name: string;
  /** The message that gets copied. */
  text: string;
}

/** A saved response as typed in Settings (a blank name uses the message's first line). */
export interface ResponseInput {
  name: string;
  text: string;
}

/** What a card on the Desk (the Canvas) shows. Mirrors `CardKind` in crates/core/src/canvas.rs. */
export type CardKind = 'entry' | 'tray' | 'note' | 'inbox';

/** One card on the Desk: where it sits and the record it shows (exactly one is set). */
export interface CanvasCard {
  kind: CardKind;
  id: string;
  x: number;
  y: number;
  z: number;
  /** The owner moved it; false means it was placed automatically this time. */
  placed: boolean;
  /** Its spot when the Desk is laid out as a timeline. */
  timelineX: number;
  timelineY: number;
  /** The box it sits in ("project:<id>" or "label:<id>"), if any. */
  inBox: string | null;
  /** The pile it's in, if any (it shows at the pile's spot). */
  stack: string | null;
  entry: Entry | null;
  tray: { item: TrayItem; date: IsoDate } | null;
  note: Note | null;
  inbox: InboxItem | null;
  /** For an entry: the same work on other days, folded into this card, newest first. */
  work: Entry[];
  /** The time across this card and `work`, as hours (only when there is other work). */
  workHours: string | null;
  /** Tickets-tray tickets with a number this work has, folded into this card. */
  folded: { item: TrayItem; date: IsoDate }[];
  /** For an entry: its tickets box, each number once across `entry`, `work` and `folded`. */
  tickets: WorkTicket[];
}

/** One ticket number in a work card's tickets box (round 14); the rule is in the app. */
export interface WorkTicket {
  number: string;
  /** Done only when every place it appears is done. */
  done: boolean;
  /** The newest note written for it. */
  note: string | null;
  /** Where it appears, newest day first; a new note goes on the first. */
  sources: TicketSource[];
}

/** One place a ticket number appears: an entry's ticket, or a tray ticket. */
export interface TicketSource {
  kind: 'entry' | 'tray';
  id: string;
  done: boolean;
  note: string | null;
  date: IsoDate;
}

/** A project's area on the Desk: cards dropped inside get its project. */
export interface CanvasArea {
  projectId: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Folded up to its name and a count. */
  collapsed: boolean;
  /** Cards inside it. */
  count: number;
  status: BoxStatus;
}

/** How a box's cards stand. Mirrors `shiftbook_core::canvas::BoxStatus`. */
export type BoxStatus = 'quiet' | 'today' | 'waiting';

/** A pile of cards on the Desk. */
export interface CanvasStack {
  id: string;
  /** Its name; null shows the count. */
  name: string | null;
  x: number;
  y: number;
  count: number;
  /**
   * The "Done today" pile Shiftbook keeps (id `done`): tickets ticked done today. It can be
   * moved (saved as the `desk.done_pile` preference) and fanned out, not renamed or filed.
   */
  done: boolean;
}

/** A drawer in the cabinet: a project's (key `project:<id>`) or a folder (`folder:<id>`). */
export interface CanvasDrawer {
  key: string;
  name: string;
  color: string | null;
  /** Cards filed in it. */
  count: number;
}

/** What Tidy into stacks did, for Undo. */
export interface TidyResult {
  stacks: string[];
  before: { kind: CardKind; id: string; x: number; y: number }[];
}

/** A plain labelled box on the Desk: it only groups cards. */
export interface CanvasLabel {
  id: string;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
  collapsed: boolean;
  count: number;
  status: BoxStatus;
}

/** One card, by what it shows and the record's id. */
export interface CardRef {
  kind: CardKind;
  id: string;
}

/** A line between two cards: automatic (with why) or drawn by hand. */
export interface CanvasLink {
  a: CardRef;
  b: CardRef;
  reason: { kind: 'ticket' | 'word'; text: string } | null;
  drawn: boolean;
}

/** Everything on the Desk. */
export interface DeskData {
  /** Bottom first. */
  cards: CanvasCard[];
  areas: CanvasArea[];
  labels: CanvasLabel[];
  stacks: CanvasStack[];
  drawers: CanvasDrawer[];
  links: CanvasLink[];
  /** The timeline's days, oldest first, with where each column starts. */
  axis: { date: IsoDate; x: number }[];
}

/**
 * A board you can step into (see crates/core/src/boards.rs): `desk`, `projects`,
 * `project:<id>`, `meetings`, `series:<id>`, `daily`, `folder:<id>` or `archive`.
 */
export interface BoardChild {
  key: string;
  title: string;
  /** A project's colour. */
  color: string | null;
  /** Cards on it (for Projects and Meetings: the boards inside). */
  count: number;
  /** The boards inside it (only filled in the tree). */
  children: BoardChild[];
}

/** A board's full screen. */
export interface BoardData {
  key: string;
  title: string;
  /** A project's colour, for the tint. */
  color: string | null;
  /** From the Desk down to this board. */
  trail: { key: string; title: string }[];
  /** It has a canvas of its own (Projects, Meetings, Archive and the Desk's list don't). */
  hasCanvas: boolean;
  children: BoardChild[];
  /** Newest first, each with its spot on this board. */
  cards: CanvasCard[];
  /** For a project: every minute logged against it, as hours. */
  hours: string | null;
  /** For a project: its tickets still open. */
  openTickets: number;
  /** An archived project. */
  archived: boolean;
  /** Every board from the Desk down, for the tree beside it. */
  tree: BoardChild[];
}

/** What brought the user back to Shiftbook. */
export type ResumeTrigger = 'unlocked' | 'break' | 'new_day';

/** The "Where was I?" card: the open wrap-up list (first item first), or else the last entry. */
export interface ResumeCard {
  trigger: ResumeTrigger;
  list: NextUpItem[];
  entry: Entry | null;
  /** When that entry last changed (UTC, RFC 3339). */
  changedAt: string | null;
}

/** Kinds of note. Mirrors `NoteKind` in crates/core/src/notes.rs. */
export type NoteKind = 'jot' | 'daily' | 'meeting' | 'page' | 'mop';

export interface Note {
  id: string;
  kind: NoteKind;
  title: string;
  body: string;
  projectId: string | null;
  tags: string[];
  /** Kept out of auto-archive. */
  keep: boolean;
  archived: boolean;
  /** Days until auto-archive moves it, when the warning should show; otherwise null. */
  archiveInDays: number | null;
  createdAt: string;
  updatedAt: string;
  /** Web addresses written in it, in order. */
  links: string[];
}

export interface NoteInput {
  kind: NoteKind;
  /** Blank uses the first line of the body. */
  title: string;
  body: string;
  projectId: string | null;
  /** Tags as typed, comma-separated. */
  tags: string;
}

export interface NoteList {
  /** Best match first when searching, else most recently changed first. */
  notes: Note[];
  /** Archived notes that match but were left out because Include archived is off. */
  hiddenArchived: number;
}

/** What Find turned up, best first. */
export interface SearchResult {
  entries: Entry[];
  inbox: InboxItem[];
  /** Notes, archived ones included. */
  notes: Note[];
  /** Tickets from the Tickets tray, done ones included. */
  tickets: TrayItem[];
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
  | 'backup_invalid'
  | 'backup_needs_secret'
  | 'backup_too_new'
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
  'backup_invalid',
  'backup_needs_secret',
  'backup_too_new',
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
  /**
   * Delete all local data (database and vault) so Shiftbook starts again at setup. Works
   * while locked. `confirm` must be the word in `DELETE_CONFIRM_WORD`. Tauri: `delete_all_data`.
   */
  deleteAllData(confirm: string): Promise<void>;

  // ----- backups (always encrypted) -----
  /**
   * Save a backup where the user chooses (a save dialog); `today` names the file. Resolves
   * with the file name, or null if cancelled. Tauri: `backup_now`.
   */
  backupNow(today: IsoDate): Promise<string | null>;
  /** The automatic and safety backups, newest first. Tauri: `list_backups`. */
  listBackups(): Promise<BackupFile[]>;
  /**
   * Choose a backup file to restore (an open dialog). Resolves with its name, or null if
   * cancelled; it is then the `PICKED_BACKUP` source. Tauri: `pick_backup_file`.
   */
  pickBackupFile(): Promise<string | null>;
  /**
   * Open a backup and say what it holds. A backup from another computer (or from before
   * Delete all data) rejects with `backup_needs_secret`: ask for the password or recovery
   * key it was made with and call again. Tauri: `check_backup`.
   */
  checkBackup(source: string, secret?: string, kind?: SecretKind): Promise<BackupSummary>;
  /**
   * Replace all data with the backup, after saving a safety copy of the current data.
   * Reload the page afterwards. Tauri: `restore_backup`.
   */
  restoreBackup(source: string, secret?: string, kind?: SecretKind): Promise<BackupSummary>;

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
  /**
   * Add work time on an entry, landing on `day` (today, or the day open on Day): the entry
   * itself when it's from that day, else that day's entry with the same title and project,
   * else a copy there. `created` means a copy was made (Undo deletes it). Tauri: `add_entry_time`.
   */
  addEntryTime(id: string, day: IsoDate, minutes: number): Promise<{ entry: Entry; created: boolean }>;
  /**
   * Log a jot as an entry on `date`: its title says what was done, the rest becomes the
   * entry's notes, ticket numbers become tickets; the jot moves to the archive. Tauri: `log_note`.
   */
  logNote(id: string, date: IsoDate, minutes: number): Promise<Entry>;
  /** The entry's notepad, saved as you type (blank clears it). */
  setEntryNotes(id: string, notes: string): Promise<Entry>;
  /** One entry (for a window showing its notepad). */
  getEntry(id: string): Promise<Entry>;
  deleteEntry(id: string): Promise<void>;
  restoreEntry(id: string): Promise<Entry>;
  addEntryTickets(id: string, text: string): Promise<Entry>;
  removeTicket(ticketId: string): Promise<Entry>;
  /** Tick an entry's ticket done, or mark it still open. Tauri: `ticket_set_done`. */
  ticketSetDone(id: string, done: boolean): Promise<Entry>;
  /**
   * Tick every copy of a ticket number on a piece of work done, or mark them all still open,
   * in one go (all change or none do). Tauri: `ticket_copies_set_done`.
   */
  ticketCopiesSetDone(entryTickets: string[], trayTickets: string[], done: boolean): Promise<void>;
  /** Write an entry ticket's notepad (blank clears it). Tauri: `ticket_set_note`. */
  ticketSetNote(id: string, note: string): Promise<Entry>;
  /** Log work: tickets or words plus a time button, on `date`. */
  quickLog(date: IsoDate, text: string, minutes: number): Promise<Entry>;

  // ----- capture -----
  /** What a capture line means (for the live chips). */
  parseCapture(text: string, today: IsoDate): Promise<Capture>;
  /** Log a capture line; it must contain hours. */
  captureLog(text: string, today: IsoDate): Promise<Entry>;

  // ----- Tickets tray -----
  trayDay(date: IsoDate): Promise<TrayDay>;
  trayAdd(date: IsoDate, text: string): Promise<TrayDay>;
  traySetMinutes(id: string, minutes: number): Promise<TrayDay>;
  /** A tray ticket's notepad (blank clears it). */
  traySetNote(id: string, note: string): Promise<TrayDay>;
  /** Tick a ticket done (also on the earlier days it came back on), or untick it: still open. */
  traySetDone(id: string, done: boolean): Promise<TrayDay>;
  /** The project a ticket's minutes count under (null = the day's "Tickets" entry). */
  traySetProject(id: string, projectId: string | null): Promise<TrayDay>;
  /** Work on a still-open ticket again on `date`: it joins that day's tray. */
  trayCarry(id: string, date: IsoDate): Promise<TrayDay>;
  /** One tray ticket, or null when it was taken out (for a window showing its notepad). */
  trayItem(id: string): Promise<TrayItem | null>;

  // ----- meetings and the bell -----
  meetingSeries(): Promise<MeetingSeries[]>;
  /** Add a series (`id` null) or change one. */
  saveMeetingSeries(id: string | null, input: SeriesInput): Promise<MeetingSeries>;
  /** Remove a series, or with `deleted` false bring it back (Undo). Its notes stay. */
  setMeetingSeriesDeleted(id: string, deleted: boolean): Promise<void>;
  /** The series' meeting note for `date`, made the first time it's asked for. */
  meetingNote(seriesId: string, date: IsoDate): Promise<Note>;
  /** What's coming up at `now` (minutes after local midnight on `today`). */
  bell(today: IsoDate, now: number): Promise<BellItem[]>;
  /** The pinned chips for the bottom bar, ones due to chase first. Tauri: `pins`. */
  pins(today: IsoDate): Promise<PinItem[]>;
  /** Pin an entry, ticket or note, or change its chase date (null clears it). Tauri: `pin`. */
  pin(kind: CardKind, id: string, chaseOn: IsoDate | null): Promise<void>;
  /** Take a chip off the bottom bar. Tauri: `unpin`. */
  unpin(kind: CardKind, id: string): Promise<void>;
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

  // ----- the Desk (Canvas) -----
  /** Everything on the Desk: cards (bottom first), project areas, links and the timeline. Tauri: `canvas`. */
  canvas(today: IsoDate): Promise<DeskData>;
  /** Put a card where it was dropped, on top. Tauri: `canvas_move`. */
  canvasMove(kind: CardKind, id: string, x: number, y: number): Promise<void>;
  /** Put a project's area here, making it the first time. Tauri: `canvas_area`. */
  canvasArea(projectId: string, x: number, y: number, w: number, h: number): Promise<void>;
  /** Take a project's area off the Desk (the project and its cards stay). Tauri: `canvas_area_remove`. */
  canvasAreaRemove(projectId: string): Promise<void>;
  /** Fold a project's area up to its name and a count, or open it. */
  canvasAreaCollapse(projectId: string, collapsed: boolean): Promise<void>;
  /** Make a labelled box (`id` null) or change one; returns its id. */
  canvasLabelSave(id: string | null, label: string, x: number, y: number, w: number, h: number, collapsed: boolean): Promise<string>;
  /** Take a labelled box off the Desk (its cards stay), or put it back. */
  canvasLabelRemove(id: string, removed: boolean): Promise<void>;
  /** Drop a card on another (shown at `onto` x, y): they pile up, or it joins that pile. */
  canvasStack(card: CardRef, onto: CardRef, ontoX: number, ontoY: number): Promise<string>;
  /** Take a card off its pile, to x, y. */
  canvasUnstack(card: CardRef, x: number, y: number): Promise<void>;
  canvasStackMove(id: string, x: number, y: number): Promise<void>;
  /** Name a pile; blank shows its count. */
  canvasStackRename(id: string, name: string): Promise<void>;
  /** Take a pile apart. */
  canvasStackDissolve(id: string): Promise<void>;
  /** File a card in a drawer (off the Desk). */
  canvasFile(card: CardRef, drawer: string): Promise<void>;
  /** File a whole pile; returns the cards filed. */
  canvasFileStack(id: string, drawer: string): Promise<CardRef[]>;
  /** Back on the Desk at x, y. */
  canvasUnfile(card: CardRef, x: number, y: number): Promise<void>;
  /** What's filed in a drawer, newest first. */
  canvasDrawer(drawer: string): Promise<CanvasCard[]>;
  /** Make a folder (`id` null) or rename one. */
  canvasFolderSave(id: string | null, name: string): Promise<{ id: string; name: string }>;
  /** Remove a folder (its cards show on the Desk while it's gone), or bring it back. */
  canvasFolderRemove(id: string, removed: boolean): Promise<void>;
  /** Tidy into stacks: pile loose cards by project. */
  canvasTidy(today: IsoDate): Promise<TidyResult>;
  /** One board's full screen (see `BoardData`). Tauri: `board`. */
  board(today: IsoDate, key: string): Promise<BoardData>;
  /** Put a card where it was dropped on a board's own canvas. Tauri: `board_move`. */
  boardMove(key: string, kind: CardKind, id: string, x: number, y: number): Promise<void>;
  /** Draw a link (`drawn`) or hide an automatic one. Tauri: `canvas_link`. */
  canvasLink(a: CardRef, b: CardRef, drawn: boolean): Promise<void>;
  /** Undo a drawn or hidden link. Tauri: `canvas_link_reset`. */
  canvasLinkReset(a: CardRef, b: CardRef): Promise<void>;

  // ----- saved responses -----
  /** Saved responses, in order. Tauri: `list_responses`. */
  listResponses(): Promise<SavedResponse[]>;
  /** Replace the list (rows with no message dropped, at most 30). Tauri: `save_responses`. */
  saveResponses(items: ResponseInput[]): Promise<SavedResponse[]>;

  // ----- next up -----
  /** The wrap-up's "next up" list, in order. */
  nextUp(): Promise<NextUpItem[]>;
  /** "Bring up tomorrow": the card comes up first on the next work day after `today`. */
  bringUp(today: IsoDate, kind: CardKind, id: string, title: string): Promise<NextUpItem>;
  /** Replace the list (blank lines dropped, at most 7); an empty list clears it. */
  setNextUp(items: string[]): Promise<NextUpItem[]>;
  tickNextUp(id: string, done: boolean): Promise<void>;
  /**
   * The "Where was I?" card for this return, or null when it shouldn't show (a short
   * break) or there's nothing to pick up. Tauri: `where_was_i`.
   */
  whereWasI(today: IsoDate, trigger: ResumeTrigger, awayMinutes: number): Promise<ResumeCard | null>;

  // ----- timer -----
  /**
   * What Stop offers to add for `seconds` counted, given the quiet stretches (seconds)
   * noticed while it ran. Tauri: `timer_offer`.
   */
  timerOffer(seconds: number, quiet: number[]): Promise<TimerOffer>;

  // ----- notes -----
  /**
   * Notes of one kind (null for all) that match `query` (blank for all). Moves notes that
   * auto-archive says are due first.
   */
  listNotes(query: string, kind: NoteKind | null, includeArchived: boolean): Promise<NoteList>;
  getNote(id: string): Promise<Note>;
  createNote(note: NoteInput): Promise<Note>;
  /** The day's Daily note (titled "Daily · Mon 19 Oct 2026"), or null. Tauri: `daily_note`. */
  dailyNote(date: IsoDate): Promise<Note | null>;
  /** Write the day's Daily note, creating it once something is typed. Tauri: `save_daily_note`. */
  saveDailyNote(date: IsoDate, body: string): Promise<Note | null>;
  updateNote(id: string, note: NoteInput): Promise<Note>;
  /** Archive, or Bring back. */
  setNoteArchived(id: string, archived: boolean): Promise<void>;
  /** Keep out of auto-archive, or let it move again. */
  setNoteKeep(id: string, keep: boolean): Promise<void>;
  setNoteDeleted(id: string, deleted: boolean): Promise<void>;

  // ----- find -----
  /** Entries, Inbox scraps, notes and the given command names that match `query`. */
  search(query: string, today: IsoDate, commands: string[]): Promise<SearchResult>;
}
