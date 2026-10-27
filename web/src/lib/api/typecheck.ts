// Compile-time check that the UI's types (DataApi.ts) are exactly the ones Rust sends. ts-rs
// writes those into ./generated (`npm run types`; CI fails if they're out of date), and any
// difference here fails `npm run check`. When a Rust type changes, run `npm run types` and
// update DataApi.ts to match. A new type the UI receives gets a line here too.
import type * as Ui from './DataApi';
import type { CommandError as GenApiErrorCode } from './generated/CommandError';
import type { AppliesTo as GenAppliesTo } from './generated/AppliesTo';
import type { AwayView as GenAwayList } from './generated/AwayView';
import type { BackupFile as GenBackupFile } from './generated/BackupFile';
import type { BackupSummary as GenBackupSummary } from './generated/BackupSummary';
import type { BellItem as GenBellItem } from './generated/BellItem';
import type { ChildView as GenBoardChild } from './generated/ChildView';
import type { BoardView as GenBoardData } from './generated/BoardView';
import type { BoxStatus as GenBoxStatus } from './generated/BoxStatus';
import type { AreaView as GenCanvasArea } from './generated/AreaView';
import type { CardView as GenCanvasCard } from './generated/CardView';
import type { DrawerView as GenCanvasDrawer } from './generated/DrawerView';
import type { LabelView as GenCanvasLabel } from './generated/LabelView';
import type { DeskLinkView as GenCanvasLink } from './generated/DeskLinkView';
import type { StackView as GenCanvasStack } from './generated/StackView';
import type { Capture as GenCapture } from './generated/Capture';
import type { CardKind as GenCardKind } from './generated/CardKind';
import type { CardRefView as GenCardRef } from './generated/CardRefView';
import type { CarriedView as GenCarriedItem } from './generated/CarriedView';
import type { CategoryCodeView as GenCategoryCode } from './generated/CategoryCodeView';
import type { CodeSource as GenCodeSource } from './generated/CodeSource';
import type { CommentFormat as GenCommentFormat } from './generated/CommentFormat';
import type { FieldView as GenCustomField } from './generated/FieldView';
import type { ValueView as GenCustomValue } from './generated/ValueView';
import type { DayPart as GenDayPart } from './generated/DayPart';
import type { DeskView as GenDeskData } from './generated/DeskView';
import type { EntryView as GenEntry } from './generated/EntryView';
import type { FieldKind as GenFieldKind } from './generated/FieldKind';
import type { FingerprintStatus as GenFingerprintStatus } from './generated/FingerprintStatus';
import type { FollowUpView as GenFollowUp } from './generated/FollowUpView';
import type { FrequencyView as GenFrequency } from './generated/FrequencyView';
import type { GroupBy as GenGroupBy } from './generated/GroupBy';
import type { InboxView as GenInboxItem } from './generated/InboxView';
import type { ItemView as GenItemSummary } from './generated/ItemView';
import type { KindCodeView as GenKindCode } from './generated/KindCodeView';
import type { MarkedView as GenMarkedDone } from './generated/MarkedView';
import type { SeriesView as GenMeetingSeries } from './generated/SeriesView';
import type { MeetingToday as GenMeetingToday } from './generated/MeetingToday';
import type { Motion as GenMotion } from './generated/Motion';
import type { NextUpView as GenNextUpItem } from './generated/NextUpView';
import type { NoteView as GenNote } from './generated/NoteView';
import type { CheckLine as GenCheckLine } from './generated/CheckLine';
import type { MopBook as GenMopBook } from './generated/MopBook';
import type { MopImport as GenMopImport } from './generated/MopImport';
import type { MopView as GenMopView } from './generated/MopView';
import type { NoteKind as GenNoteKind } from './generated/NoteKind';
import type { NoteListView as GenNoteList } from './generated/NoteListView';
import type { PastTimeView as GenPastTime } from './generated/PastTimeView';
import type { PausedTimer as GenPausedTimer } from './generated/PausedTimer';
import type { Launch as GenPhoneLaunch } from './generated/Launch';
import type { PinView as GenPinItem } from './generated/PinView';
import type { PinOutcome as GenPinOutcome } from './generated/PinOutcome';
import type { Prefs as GenPrefs } from './generated/Prefs';
import type { ProjectView as GenProject } from './generated/ProjectView';
import type { ProjectEffort as GenProjectEffort } from './generated/ProjectEffort';
import type { RecurringView as GenRecurring } from './generated/RecurringView';
import type { RecurringState as GenRecurringState } from './generated/RecurringState';
import type { ResolvedCode as GenResolvedCode } from './generated/ResolvedCode';
import type { ResumeView as GenResumeCard } from './generated/ResumeView';
import type { ResumeTrigger as GenResumeTrigger } from './generated/ResumeTrigger';
import type { RibbonDay as GenRibbonDay } from './generated/RibbonDay';
import type { RibbonWeek as GenRibbonWeek } from './generated/RibbonWeek';
import type { SameWorkView as GenSameWork } from './generated/SameWorkView';
import type { ResponseView as GenSavedResponse } from './generated/ResponseView';
import type { SearchView as GenSearchResult } from './generated/SearchView';
import type { SheetDay as GenSheetDay } from './generated/SheetDay';
import type { SheetItem as GenSheetItem } from './generated/SheetItem';
import type { SheetLine as GenSheetLine } from './generated/SheetLine';
import type { SheetWarning as GenSheetWarning } from './generated/SheetWarning';
import type { ListView as GenShopList } from './generated/ListView';
import type { SyncReport as GenSyncReport } from './generated/SyncReport';
import type { Theme as GenTheme } from './generated/Theme';
import type { TickView as GenTick } from './generated/TickView';
import type { TicketView as GenTicket } from './generated/TicketView';
import type { LinkView as GenTicketLink } from './generated/LinkView';
import type { TidyView as GenTidyResult } from './generated/TidyView';
import type { TimerOffer as GenTimerOffer } from './generated/TimerOffer';
import type { CodeView as GenTimesheetCode } from './generated/CodeView';
import type { TrayDayView as GenTrayDay } from './generated/TrayDayView';
import type { TrayItemView as GenTrayItem } from './generated/TrayItemView';
import type { UnlockOutcome as GenUnlockOutcome } from './generated/UnlockOutcome';
import type { UseMode as GenUseMode } from './generated/UseMode';
import type { VaultStatus as GenVaultStatus } from './generated/VaultStatus';
import type { WeekSheet as GenWeekSheet } from './generated/WeekSheet';
import type { WindowMode as GenWindowMode } from './generated/WindowMode';
import type { WindowSpot as GenWindowSpot } from './generated/WindowSpot';

/** True only when A and B are the same type. */
type Same<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
/** Fails to compile unless given `true`. */
type Assert<T extends true> = T;

export type Checks = [
  // The UI adds 'unknown' for anything it doesn't recognise.
  Assert<Same<Exclude<Ui.ApiErrorCode, 'unknown'>, GenApiErrorCode>>,
  Assert<Same<Ui.AppliesTo, GenAppliesTo>>,
  Assert<Same<Ui.AwayList, GenAwayList>>,
  Assert<Same<Ui.BackupFile, GenBackupFile>>,
  Assert<Same<Ui.BackupSummary, GenBackupSummary>>,
  Assert<Same<Ui.BellItem, GenBellItem>>,
  Assert<Same<Ui.BoardChild, GenBoardChild>>,
  Assert<Same<Ui.BoardData, GenBoardData>>,
  Assert<Same<Ui.BoxStatus, GenBoxStatus>>,
  Assert<Same<Ui.CanvasArea, GenCanvasArea>>,
  Assert<Same<Ui.CanvasCard, GenCanvasCard>>,
  Assert<Same<Ui.CanvasDrawer, GenCanvasDrawer>>,
  Assert<Same<Ui.CanvasLabel, GenCanvasLabel>>,
  Assert<Same<Ui.CanvasLink, GenCanvasLink>>,
  Assert<Same<Ui.CanvasStack, GenCanvasStack>>,
  Assert<Same<Ui.Capture, GenCapture>>,
  Assert<Same<Ui.CardKind, GenCardKind>>,
  Assert<Same<Ui.CardRef, GenCardRef>>,
  Assert<Same<Ui.CarriedItem, GenCarriedItem>>,
  Assert<Same<Ui.CategoryCode, GenCategoryCode>>,
  Assert<Same<Ui.CodeSource, GenCodeSource>>,
  Assert<Same<Ui.CommentFormat, GenCommentFormat>>,
  Assert<Same<Ui.CustomField, GenCustomField>>,
  Assert<Same<Ui.CustomValue, GenCustomValue>>,
  Assert<Same<Ui.DayPart, GenDayPart>>,
  Assert<Same<Ui.DeskData, GenDeskData>>,
  Assert<Same<Ui.Entry, GenEntry>>,
  Assert<Same<Ui.FieldKind, GenFieldKind>>,
  Assert<Same<Ui.FingerprintStatus, GenFingerprintStatus>>,
  Assert<Same<Ui.FollowUp, GenFollowUp>>,
  Assert<Same<Ui.Frequency, GenFrequency>>,
  Assert<Same<Ui.GroupBy, GenGroupBy>>,
  Assert<Same<Ui.InboxItem, GenInboxItem>>,
  Assert<Same<Ui.ItemSummary, GenItemSummary>>,
  Assert<Same<Ui.KindCode, GenKindCode>>,
  Assert<Same<Ui.MarkedDone, GenMarkedDone>>,
  Assert<Same<Ui.MeetingSeries, GenMeetingSeries>>,
  Assert<Same<Ui.MeetingToday, GenMeetingToday>>,
  Assert<Same<Ui.Motion, GenMotion>>,
  Assert<Same<Ui.NextUpItem, GenNextUpItem>>,
  Assert<Same<Ui.Note, GenNote>>,
  Assert<Same<Ui.CheckLine, GenCheckLine>>,
  Assert<Same<Ui.MopBook, GenMopBook>>,
  Assert<Same<Ui.MopImport, GenMopImport>>,
  Assert<Same<Ui.MopView, GenMopView>>,
  Assert<Same<Ui.NoteKind, GenNoteKind>>,
  Assert<Same<Ui.NoteList, GenNoteList>>,
  Assert<Same<Ui.PastTime, GenPastTime>>,
  Assert<Same<Ui.PausedTimer, GenPausedTimer>>,
  Assert<Same<Ui.PhoneLaunch, GenPhoneLaunch>>,
  Assert<Same<Ui.PinItem, GenPinItem>>,
  Assert<Same<Ui.PinOutcome, GenPinOutcome>>,
  Assert<Same<Ui.Prefs, GenPrefs>>,
  Assert<Same<Ui.Project, GenProject>>,
  Assert<Same<Ui.ProjectEffort, GenProjectEffort>>,
  Assert<Same<Ui.Recurring, GenRecurring>>,
  Assert<Same<Ui.RecurringState, GenRecurringState>>,
  Assert<Same<Ui.ResolvedCode, GenResolvedCode>>,
  Assert<Same<Ui.ResumeCard, GenResumeCard>>,
  Assert<Same<Ui.ResumeTrigger, GenResumeTrigger>>,
  Assert<Same<Ui.RibbonDay, GenRibbonDay>>,
  Assert<Same<Ui.RibbonWeek, GenRibbonWeek>>,
  Assert<Same<Ui.SameWork, GenSameWork>>,
  Assert<Same<Ui.SavedResponse, GenSavedResponse>>,
  Assert<Same<Ui.SearchResult, GenSearchResult>>,
  Assert<Same<Ui.SheetDay, GenSheetDay>>,
  Assert<Same<Ui.SheetItem, GenSheetItem>>,
  Assert<Same<Ui.SheetLine, GenSheetLine>>,
  Assert<Same<Ui.SheetWarning, GenSheetWarning>>,
  Assert<Same<Ui.ShopList, GenShopList>>,
  Assert<Same<Ui.SyncReport, GenSyncReport>>,
  Assert<Same<Ui.Theme, GenTheme>>,
  Assert<Same<Ui.Tick, GenTick>>,
  Assert<Same<Ui.Ticket, GenTicket>>,
  Assert<Same<Ui.TicketLink, GenTicketLink>>,
  Assert<Same<Ui.TidyResult, GenTidyResult>>,
  Assert<Same<Ui.TimerOffer, GenTimerOffer>>,
  Assert<Same<Ui.TimesheetCode, GenTimesheetCode>>,
  Assert<Same<Ui.TrayDay, GenTrayDay>>,
  Assert<Same<Ui.TrayItem, GenTrayItem>>,
  Assert<Same<Ui.UnlockOutcome, GenUnlockOutcome>>,
  Assert<Same<Ui.UseMode, GenUseMode>>,
  Assert<Same<Ui.VaultStatus, GenVaultStatus>>,
  Assert<Same<Ui.WeekSheet, GenWeekSheet>>,
  Assert<Same<Ui.WindowMode, GenWindowMode>>,
  Assert<Same<Ui.WindowSpot, GenWindowSpot>>,
];
