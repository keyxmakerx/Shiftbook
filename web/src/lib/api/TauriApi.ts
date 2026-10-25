// Phase 1 implementation of DataApi. This is the ONLY file in web/ allowed to import
// from @tauri-apps/* (CLAUDE.md rule 1). Argument names are camelCase here; Tauri maps
// them to the commands' snake_case parameters.
import { invoke } from '@tauri-apps/api/core';
import { toApiError } from './DataApi';
import type {
  AwayList,
  Frequency,
  MarkedDone,
  Recurring,
  RecurringInput,
  ShopList,
  UseMode,
  AppInfo,
  BackupFile,
  BackupSummary,
  Capture,
  PhoneLaunch,
  FingerprintStatus,
  SyncReport,
  CategoryCode,
  CodeInput,
  CustomField,
  CustomValue,
  DataApi,
  Entry,
  EntryInput,
  FieldInput,
  GroupBy,
  InboxItem,
  NextUpItem,
  MeetingSeries,
  SeriesInput,
  BellItem,
  TrayItem,
  CardRef,
  BoardData,
  DeskData,
  PinItem,
  CardKind,
  CanvasCard,
  TidyResult,
  Note,
  NoteInput,
  NoteKind,
  NoteList,
  ResponseInput,
  SavedResponse,
  ResumeCard,
  ResumeTrigger,
  SearchResult,
  IsoDate,
  PrefKey,
  Prefs,
  Project,
  ProjectInput,
  RibbonWeek,
  SecretKind,
  Tick,
  TimerOffer,
  TimesheetCode,
  TrayDay,
  UnlockOutcome,
  VaultStatus,
  WeekSheet,
} from './DataApi';

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
    return call('app_info');
  }
  vaultStatus(): Promise<VaultStatus> {
    return call('vault_status');
  }
  vaultSetup(password: string): Promise<string> {
    return call('vault_setup', { password });
  }
  vaultConfirmRecoverySaved(): Promise<void> {
    return call('vault_confirm_recovery_saved');
  }
  unlock(secret: string, kind: SecretKind): Promise<UnlockOutcome> {
    return call('unlock', { secret, kind });
  }
  lock(): Promise<void> {
    return call('lock');
  }
  changePassword(currentPassword: string, newPassword: string): Promise<void> {
    return call('change_password', { currentPassword, newPassword });
  }
  setNewPassword(newPassword: string): Promise<void> {
    return call('set_new_password', { newPassword });
  }
  regenerateRecoveryKey(password: string): Promise<string> {
    return call('regenerate_recovery_key', { password });
  }
  deleteAllData(confirm: string): Promise<void> {
    return call('delete_all_data', { confirm });
  }

  backupNow(today: IsoDate): Promise<string | null> {
    return call('backup_now', { today });
  }
  listBackups(): Promise<BackupFile[]> {
    return call('list_backups');
  }
  pickBackupFile(): Promise<string | null> {
    return call('pick_backup_file');
  }
  checkBackup(source: string, secret?: string, kind?: SecretKind): Promise<BackupSummary> {
    return call('check_backup', { source, secret: secret ?? null, kind: kind ?? null });
  }
  restoreBackup(source: string, secret?: string, kind?: SecretKind): Promise<BackupSummary> {
    return call('restore_backup', { source, secret: secret ?? null, kind: kind ?? null });
  }

  allowCapture(): Promise<number> {
    return call('allow_capture');
  }
  protectCapture(): Promise<void> {
    return call('protect_capture');
  }

  loadPrefs(): Promise<Prefs> {
    return call('load_prefs');
  }
  savePref(key: PrefKey, value: string): Promise<Prefs> {
    return call('save_pref', { key, value });
  }

  listProjects(): Promise<Project[]> {
    return call('list_projects');
  }
  createProject(name: string, color: string | null): Promise<Project> {
    return call('create_project', { name, color });
  }
  updateProject(id: string, input: ProjectInput): Promise<Project> {
    return call('update_project', { id, input });
  }

  listEntries(from: IsoDate, to: IsoDate): Promise<Entry[]> {
    return call('list_entries', { from, to });
  }
  createEntry(input: EntryInput, tickets: string): Promise<Entry> {
    return call('create_entry', { input, tickets });
  }
  updateEntry(id: string, input: EntryInput): Promise<Entry> {
    return call('update_entry', { id, input });
  }
  setEntryNotes(id: string, notes: string): Promise<Entry> {
    return call('set_entry_notes', { id, notes });
  }
  getEntry(id: string): Promise<Entry> {
    return call('get_entry', { id });
  }
  addEntryMinutes(id: string, delta: number): Promise<Entry> {
    return call('add_entry_minutes', { id, delta });
  }  addEntryTime(id: string, day: IsoDate, minutes: number): Promise<{ entry: Entry; created: boolean }> {
    return call('add_entry_time', { id, day, minutes });
  }  logNote(id: string, date: IsoDate, minutes: number): Promise<Entry> {
    return call('log_note', { id, date, minutes });
  }


  deleteEntry(id: string): Promise<void> {
    return call('delete_entry', { id });
  }
  restoreEntry(id: string): Promise<Entry> {
    return call('restore_entry', { id });
  }
  addEntryTickets(id: string, text: string): Promise<Entry> {
    return call('add_entry_tickets', { id, text });
  }
  removeTicket(ticketId: string): Promise<Entry> {
    return call('remove_ticket', { id: ticketId });
  }  ticketSetDone(id: string, done: boolean): Promise<Entry> {
    return call('ticket_set_done', { id, done });
  }
  ticketSetNote(id: string, note: string): Promise<Entry> {
    return call('ticket_set_note', { id, note });
  }

  quickLog(date: IsoDate, text: string, minutes: number): Promise<Entry> {
    return call('quick_log', { date, text, minutes });
  }

  parseCapture(text: string, today: IsoDate): Promise<Capture> {
    return call('parse_capture_line', { text, today });
  }
  captureLog(text: string, today: IsoDate): Promise<Entry> {
    return call('capture_log', { text, today });
  }

  trayDay(date: IsoDate): Promise<TrayDay> {
    return call('tray_day', { date });
  }
  trayAdd(date: IsoDate, text: string): Promise<TrayDay> {
    return call('tray_add', { date, text });
  }
  traySetMinutes(id: string, minutes: number): Promise<TrayDay> {
    return call('tray_set_minutes', { id, minutes });
  }
  traySetNote(id: string, note: string): Promise<TrayDay> {
    return call('tray_set_note', { id, note });
  }
  traySetDone(id: string, done: boolean): Promise<TrayDay> {
    return call('tray_set_done', { id, done });
  }
  traySetProject(id: string, projectId: string | null): Promise<TrayDay> {
    return call('tray_set_project', { id, projectId });
  }
  trayCarry(id: string, date: IsoDate): Promise<TrayDay> {
    return call('tray_carry', { id, date });
  }
  trayItem(id: string): Promise<TrayItem | null> {
    return call('tray_item', { id });
  }
  meetingSeries(): Promise<MeetingSeries[]> {
    return call('meeting_series');
  }
  logMeeting(seriesId: string, today: IsoDate, minutes: number): Promise<Entry> {
    return call('log_meeting', { seriesId, today, minutes });
  }
  skipMeeting(seriesId: string, today: IsoDate, skip: boolean): Promise<void> {
    return call('skip_meeting', { seriesId, today, skip });
  }
  saveMeetingSeries(id: string | null, input: SeriesInput): Promise<MeetingSeries> {
    return call('save_meeting_series', { id, input });
  }
  setMeetingSeriesDeleted(id: string, deleted: boolean): Promise<void> {
    return call('set_meeting_series_deleted', { id, deleted });
  }
  meetingNote(seriesId: string, date: IsoDate): Promise<Note> {
    return call('meeting_note', { seriesId, date });
  }
  bell(today: IsoDate, now: number): Promise<BellItem[]> {
    return call('bell', { today, now });
  }
  pins(today: IsoDate): Promise<PinItem[]> {
    return call('pins', { today });
  }
  pin(kind: CardKind, id: string, chaseOn: IsoDate | null): Promise<void> {
    return call('pin', { kind, id, chaseOn });
  }
  unpin(kind: CardKind, id: string): Promise<void> {
    return call('unpin', { kind, id });
  }

  trayRemove(id: string): Promise<TrayDay> {
    return call('tray_remove', { id });
  }
  trayRestore(id: string): Promise<TrayDay> {
    return call('tray_restore', { id });
  }

  ribbon(today: IsoDate): Promise<RibbonWeek[]> {
    return call('ribbon', { today });
  }
  weekSheet(date: IsoDate, today: IsoDate, groupBy: GroupBy | null): Promise<WeekSheet> {
    return call('week_sheet', { date, today, groupBy });
  }
  listTicks(weekStart: IsoDate): Promise<Tick[]> {
    return call('list_ticks', { weekStart });
  }
  setTick(weekStart: IsoDate, day: IsoDate, lineKey: string, done: boolean): Promise<void> {
    return call('set_tick', { weekStart, day, lineKey, done });
  }

  listTimesheetCodes(): Promise<TimesheetCode[]> {
    return call('list_timesheet_codes');
  }
  saveTimesheetCode(id: string | null, input: CodeInput): Promise<TimesheetCode> {
    return call('save_timesheet_code', { id, input });
  }
  categoryCodes(): Promise<CategoryCode[]> {
    return call('category_codes');
  }
  setCategoryCode(category: string, codeId: string | null): Promise<void> {
    return call('set_category_code', { category, codeId });
  }

  listCustomFields(): Promise<CustomField[]> {
    return call('list_custom_fields');
  }
  saveCustomField(id: string | null, input: FieldInput): Promise<CustomField> {
    return call('save_custom_field', { id, input });
  }
  deleteCustomField(id: string): Promise<void> {
    return call('delete_custom_field', { id });
  }
  listCustomValues(recordId: string): Promise<CustomValue[]> {
    return call('list_custom_values', { recordId });
  }
  setCustomValue(fieldId: string, recordId: string, value: string): Promise<string> {
    return call('set_custom_value', { fieldId, recordId, value });
  }

  listInbox(): Promise<InboxItem[]> {
    return call('list_inbox');
  }
  addInbox(text: string): Promise<InboxItem> {
    return call('add_inbox', { text });
  }
  setInboxDeleted(id: string, deleted: boolean): Promise<void> {
    return call('set_inbox_deleted', { id, deleted });
  }

  canvas(today: IsoDate): Promise<DeskData> {
    return call('canvas', { today });
  }
  canvasMove(kind: CardKind, id: string, x: number, y: number): Promise<void> {
    return call('canvas_move', { kind, id, x, y });
  }
  canvasArea(projectId: string, x: number, y: number, w: number, h: number): Promise<void> {
    return call('canvas_area', { projectId, x, y, w, h });
  }
  canvasAreaRemove(projectId: string): Promise<void> {
    return call('canvas_area_remove', { projectId });
  }
  canvasAreaCollapse(projectId: string, collapsed: boolean): Promise<void> {
    return call('canvas_area_collapse', { projectId, collapsed });
  }
  canvasLabelSave(id: string | null, label: string, x: number, y: number, w: number, h: number, collapsed: boolean): Promise<string> {
    return call('canvas_label_save', { id, label, x, y, w, h, collapsed });
  }
  canvasLabelRemove(id: string, removed: boolean): Promise<void> {
    return call('canvas_label_remove', { id, removed });
  }
  canvasStack(card: CardRef, onto: CardRef, ontoX: number, ontoY: number): Promise<string> {
    return call('canvas_stack', { kind: card.kind, id: card.id, ontoKind: onto.kind, ontoId: onto.id, ontoX, ontoY });
  }
  canvasUnstack(card: CardRef, x: number, y: number): Promise<void> {
    return call('canvas_unstack', { kind: card.kind, id: card.id, x, y });
  }
  canvasStackMove(id: string, x: number, y: number): Promise<void> {
    return call('canvas_stack_move', { id, x, y });
  }
  canvasStackRename(id: string, name: string): Promise<void> {
    return call('canvas_stack_rename', { id, name });
  }
  canvasStackDissolve(id: string): Promise<void> {
    return call('canvas_stack_dissolve', { id });
  }
  canvasFile(card: CardRef, drawer: string): Promise<void> {
    return call('canvas_file', { kind: card.kind, id: card.id, drawer });
  }
  canvasFileStack(id: string, drawer: string): Promise<CardRef[]> {
    return call('canvas_file_stack', { id, drawer });
  }
  canvasUnfile(card: CardRef, x: number, y: number): Promise<void> {
    return call('canvas_unfile', { kind: card.kind, id: card.id, x, y });
  }
  canvasDrawer(drawer: string): Promise<CanvasCard[]> {
    return call('canvas_drawer', { drawer });
  }
  canvasFolderSave(id: string | null, name: string): Promise<{ id: string; name: string }> {
    return call('canvas_folder_save', { id, name });
  }
  canvasFolderRemove(id: string, removed: boolean): Promise<void> {
    return call('canvas_folder_remove', { id, removed });
  }
  canvasTidy(today: IsoDate): Promise<TidyResult> {
    return call('canvas_tidy', { today });
  }
  board(today: IsoDate, key: string): Promise<BoardData> {
    return call('board', { today, key });
  }
  boardMove(key: string, kind: CardKind, id: string, x: number, y: number): Promise<void> {
    return call('board_move', { key, kind, id, x, y });
  }
  canvasLink(a: CardRef, b: CardRef, drawn: boolean): Promise<void> {
    return call('canvas_link', { aKind: a.kind, aId: a.id, bKind: b.kind, bId: b.id, drawn });
  }
  canvasLinkReset(a: CardRef, b: CardRef): Promise<void> {
    return call('canvas_link_reset', { aKind: a.kind, aId: a.id, bKind: b.kind, bId: b.id });
  }

  listResponses(): Promise<SavedResponse[]> {
    return call('list_responses');
  }
  saveResponses(items: ResponseInput[]): Promise<SavedResponse[]> {
    return call('save_responses', { items });
  }

  nextUp(): Promise<NextUpItem[]> {
    return call('next_up');
  }
  setNextUp(items: string[]): Promise<NextUpItem[]> {
    return call('set_next_up', { items });
  }
  bringUp(today: IsoDate, kind: CardKind, id: string, title: string, on?: IsoDate): Promise<NextUpItem> {
    return call('bring_up', { today, kind, id, title, on: on ?? null });
  }
  pastedTickets(text: string): Promise<string[] | null> {
    return call('pasted_tickets', { text });
  }
  findDay(text: string, today: IsoDate): Promise<IsoDate | null> {
    return call('find_day', { text, today });
  }
  standup(today: IsoDate): Promise<string> {
    return call('standup', { today });
  }
  bringUpDays(today: IsoDate): Promise<IsoDate[]> {
    return call('bring_up_days', { today });
  }
  tickNextUp(id: string, done: boolean): Promise<void> {
    return call('tick_next_up', { id, done });
  }
  whereWasI(today: IsoDate, trigger: ResumeTrigger, awayMinutes: number): Promise<ResumeCard | null> {
    return call('where_was_i', { today, trigger, awayMinutes });
  }
  timerOffer(seconds: number, quiet: number[]): Promise<TimerOffer> {
    return call('timer_offer', { seconds, quiet });
  }
  timerAwayQuestion(seconds: number): Promise<number | null> {
    return call('timer_away_question', { seconds });
  }

  phoneLaunch(): Promise<PhoneLaunch | null> {
    return call('phone_launch');
  }
  phoneAwaySeconds(): Promise<number> {
    return call('phone_away_seconds');
  }
  phoneTimerAway(on: boolean, seconds: number): Promise<void> {
    return call('phone_timer_away', { on, seconds });
  }
  phoneAllowNotifications(): Promise<boolean> {
    return call('phone_allow_notifications');
  }
  planNudges(today: IsoDate, now: number): Promise<void> {
    return call('plan_nudges', { today, now });
  }
  lastSwap(): Promise<string | null> {
    return call('last_swap');
  }
  makeSyncFile(today: IsoDate): Promise<string | null> {
    return call('make_sync_file', { today });
  }
  pickSyncFile(): Promise<string | null> {
    return call('pick_sync_file');
  }
  checkSyncFile(secret?: string, kind?: SecretKind): Promise<SyncReport> {
    return call('check_sync_file', { secret: secret ?? null, kind: kind ?? null });
  }
  bringInSyncFile(secret?: string, kind?: SecretKind): Promise<SyncReport> {
    return call('bring_in_sync_file', { secret: secret ?? null, kind: kind ?? null });
  }
  fingerprintStatus(): Promise<FingerprintStatus> {
    return call('fingerprint_status');
  }
  fingerprintOn(password: string): Promise<void> {
    return call('fingerprint_on', { password });
  }
  fingerprintOff(): Promise<void> {
    return call('fingerprint_off');
  }
  fingerprintUnlock(): Promise<UnlockOutcome> {
    return call('fingerprint_unlock');
  }

  listNotes(query: string, kind: NoteKind | null, includeArchived: boolean): Promise<NoteList> {
    return call('list_notes', { query, kind, includeArchived });
  }
  getNote(id: string): Promise<Note> {
    return call('get_note', { id });
  }
  createNote(note: NoteInput): Promise<Note> {
    return call('create_note', { note });
  }
  dailyNote(date: IsoDate): Promise<Note | null> {
    return call('daily_note', { date });
  }
  saveDailyNote(date: IsoDate, body: string): Promise<Note | null> {
    return call('save_daily_note', { date, body });
  }
  updateNote(id: string, note: NoteInput): Promise<Note> {
    return call('update_note', { id, note });
  }
  setNoteArchived(id: string, archived: boolean): Promise<void> {
    return call('set_note_archived', { id, archived });
  }
  setNoteKeep(id: string, keep: boolean): Promise<void> {
    return call('set_note_keep', { id, keep });
  }
  setNoteDeleted(id: string, deleted: boolean): Promise<void> {
    return call('set_note_deleted', { id, deleted });
  }

  search(query: string, today: IsoDate, commands: string[]): Promise<SearchResult> {
    return call('search', { query, today, commands });
  }

  recurring(today: IsoDate): Promise<Recurring[]> {
    return call('recurring_list', { today });
  }
  saveRecurring(id: string | null, input: RecurringInput): Promise<void> {
    return call('recurring_save', { id, input });
  }
  recurringDone(id: string, date: IsoDate): Promise<MarkedDone> {
    return call('recurring_done', { id, date });
  }
  recurringRenewed(id: string, expires: IsoDate, date: IsoDate): Promise<MarkedDone> {
    return call('recurring_renewed', { id, expires, date });
  }
  recurringUndo(marked: MarkedDone): Promise<void> {
    return call('recurring_undo', { marked });
  }
  setRecurringDeleted(id: string, deleted: boolean): Promise<void> {
    return call('recurring_set_deleted', { id, deleted });
  }
  recurringChoices(): Promise<{ frequencies: Frequency[]; kinds: string[] }> {
    return call('recurring_choices');
  }

  lists(): Promise<ShopList[]> {
    return call('lists');
  }
  createList(title: string, items: string): Promise<ShopList> {
    return call('list_create', { title, items });
  }
  renameList(id: string, title: string): Promise<ShopList> {
    return call('list_rename', { id, title });
  }
  setListDeleted(id: string, deleted: boolean): Promise<void> {
    return call('list_set_deleted', { id, deleted });
  }
  addListItems(id: string, text: string): Promise<ShopList> {
    return call('list_add_items', { id, text });
  }
  listItemDone(itemId: string, done: boolean): Promise<ShopList> {
    return call('list_item_done', { id: itemId, done });
  }
  listItemText(itemId: string, text: string): Promise<ShopList> {
    return call('list_item_text', { id: itemId, text });
  }
  listItemDeleted(itemId: string, deleted: boolean): Promise<ShopList> {
    return call('list_item_deleted', { id: itemId, deleted });
  }
  resetList(id: string): Promise<ShopList> {
    return call('list_reset', { id });
  }

  setUseMode(mode: UseMode): Promise<Prefs> {
    return call('set_use_mode', { mode });
  }
  whileAway(today: IsoDate, now: number): Promise<AwayList> {
    return call('while_away', { today, now });
  }
}
