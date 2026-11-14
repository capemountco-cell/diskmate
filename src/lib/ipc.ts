/**
 * Typed access to the engine.
 *
 * Every call the UI makes to Rust goes through this module. Components never
 * call `invoke` directly, so the command surface stays visible in one place and
 * the argument shapes cannot drift apart across features.
 *
 * When Mr Clean's window is opened by Vite alone (`npm run dev:web`), the Tauri
 * bridge is absent. That case reports itself plainly rather than substituting
 * sample data: a storage tool showing invented numbers would be worse than one
 * showing none.
 */

import { invoke } from "@tauri-apps/api/core";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AgeBucket,
  Application,
  AppInfo,
  CandidateGroup,
  CandidateState,
  ChildPage,
  ChildQuery,
  CleanupCandidate,
  Comparison,
  Confirmation,
  DuplicateGroup,
  DuplicateProgress,
  DuplicateStats,
  EntryRow,
  Leftover,
  MemoryReport,
  MonitorReport,
  ProcessMemory,
  RemovalOutcome,
  RemovalReport,
  Sample,
  ScanFinished,
  ScanProgress,
  ScanSummary,
  Snapshot,
  StartupReport,
  StorageSnapshot,
  VolumeInfo,
} from "./types";

export const EVENT_SCAN_PROGRESS = "diskmate://scan-progress";
export const EVENT_SCAN_FINISHED = "diskmate://scan-finished";
export const EVENT_DUPLICATE_PROGRESS = "diskmate://duplicate-progress";
export const EVENT_DUPLICATE_FINISHED = "diskmate://duplicate-finished";
export const EVENT_DUPLICATE_FAILED = "diskmate://duplicate-failed";
export const EVENT_REMOVAL_PROGRESS = "diskmate://removal-progress";

/** How far moving the approved queue to the Trash has got. */
export interface RemovalProgress {
  done: number;
  total: number;
  doneBytes: number;
  totalBytes: number;
  current: string | null;
}

/** Subscribe to removal progress, sent after every batch moved to the Trash. */
export function onRemovalProgress(
  handler: (progress: RemovalProgress) => void,
): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<RemovalProgress>(EVENT_REMOVAL_PROGRESS, (event) => handler(event.payload));
}

/** True when the app is running inside its Tauri window. */
export function engineAvailable(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export class EngineUnavailableError extends Error {
  constructor() {
    super(
      "Mr Clean's engine is not running. Start the app with `npm run dev` rather than " +
        "opening the web build directly.",
    );
    this.name = "EngineUnavailableError";
  }
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!engineAvailable()) throw new EngineUnavailableError();
  return invoke<T>(command, args);
}

export const engine = {
  appInfo: () => call<AppInfo>("app_info"),
  listVolumes: () => call<VolumeInfo[]>("list_volumes"),

  startScan: (root: string) => call<{ root: string }>("start_scan", { root }),
  cancelScan: () => call<boolean>("cancel_scan"),
  activeScan: () => call<string | null>("active_scan"),

  latestScan: (root: string) => call<ScanSummary | null>("latest_scan", { root }),
  recentScans: (limit: number) => call<ScanSummary[]>("recent_scans", { limit }),
  storageSnapshot: (scanId: number) => call<StorageSnapshot>("storage_snapshot", { scanId }),

  listChildren: (query: ChildQuery) => call<ChildPage>("list_children", { query }),
  entryAncestors: (scanId: number, entryId: number) =>
    call<EntryRow[]>("entry_ancestors", { scanId, entryId }),
  directoryByPath: (scanId: number, path: string) =>
    call<EntryRow | null>("directory_by_path", { scanId, path }),

  memoryReport: () => call<MemoryReport>("memory_report"),
  topApps: (limit: number) => call<ProcessMemory[]>("top_apps", { limit }),
  // The one process action: a normal quit request to an app the person
  // clicked. The engine refuses if that pid is no longer the named app.
  quitApp: (pid: number, name: string) => call<void>("quit_app", { pid, name }),
  startupReport: () => call<StartupReport>("startup_report"),

  // Visualisations. Both are bounded queries, not tree dumps.
  subtree: (scanId: number, rootId: number, maxDepth: number, fanOut: number) =>
    call<EntryRow[]>("subtree", { scanId, rootId, maxDepth, fanOut }),
  ageBreakdown: (scanId: number) => call<AgeBucket[]>("age_breakdown", { scanId }),

  // Cleanup. Only `removeApproved` removes anything, and the engine checks the
  // confirmation against the queue before it does.
  proposeCleanup: (scanId: number) => call<CleanupCandidate[]>("propose_cleanup", { scanId }),
  cleanupGroups: (scanId: number) => call<CandidateGroup[]>("cleanup_groups", { scanId }),
  cleanupCandidates: (scanId: number, stateFilter: CandidateState | null, limit: number) =>
    call<CleanupCandidate[]>("cleanup_candidates", { scanId, stateFilter, limit }),
  stageCleanup: (ids: number[]) => call<number>("stage_cleanup", { ids }),
  unstageCleanup: (ids: number[]) => call<number>("unstage_cleanup", { ids }),
  stagedCleanup: () => call<CleanupCandidate[]>("staged_cleanup"),
  removeApproved: (confirmation: Confirmation) =>
    call<RemovalReport>("remove_approved", { confirmation }),
  cleanupHistory: (limit: number) => call<RemovalOutcome[]>("cleanup_history", { limit }),

  // Duplicates.
  startDuplicateSearch: (scanId: number) => call<void>("start_duplicate_search", { scanId }),
  cancelDuplicateSearch: () => call<void>("cancel_duplicate_search"),
  duplicateGroups: (scanId: number, limit: number) =>
    call<DuplicateGroup[]>("duplicate_groups", { scanId, limit }),

  // Applications and the uninstaller.
  inspectApplications: () => call<Application[]>("inspect_applications"),
  applications: () => call<Application[]>("applications"),
  applicationLeftovers: (applicationId: number) =>
    call<Leftover[]>("application_leftovers", { applicationId }),
  proposeUninstall: (applicationId: number) =>
    call<CleanupCandidate[]>("propose_uninstall", { applicationId }),

  // Snapshots.
  captureSnapshot: (scanId: number, label: string) =>
    call<Snapshot>("capture_snapshot", { scanId, label }),
  snapshots: (limit: number) => call<Snapshot[]>("snapshots", { limit }),
  deleteSnapshot: (id: number) => call<void>("delete_snapshot", { id }),
  compareSnapshots: (baseId: number, targetId: number) =>
    call<Comparison>("compare_snapshots", { baseId, targetId }),

  // The live monitor.
  monitorStart: (intervalMs: number, watchPath: string | null) =>
    call<void>("monitor_start", { intervalMs, watchPath }),
  monitorStop: () => call<void>("monitor_stop"),
  monitorReport: () => call<MonitorReport>("monitor_report"),
  monitorSample: () => call<Sample>("monitor_sample"),

  /** The app's icon as a PNG data URL, or null when it has none. */
  appIcon: (path: string, size: number) => call<string | null>("app_icon", { path, size }),

  // Windows: the menu bar panel and the app.
  openMainWindow: () => call<void>("open_main_window"),
  hidePanel: () => call<void>("hide_panel"),
  quitMrClean: () => call<void>("quit_mr_clean"),

  // Wipe-down mode: the keyboard and trackpad ignore everything for a while.
  wipePermission: () => call<boolean>("wipe_permission"),
  wipeRequestPermission: () => call<boolean>("wipe_request_permission"),
  wipeStart: (seconds: number) => call<void>("wipe_start", { seconds }),
};

/** What `wipeStart` rejects with when Accessibility permission is missing. */
export const WIPE_NEEDS_PERMISSION = "accessibility";

/** Wipe-down mode's events, from the engine and from the menu bar panel. */
export function onWipe(handlers: {
  started?: (seconds: number) => void;
  held?: (fraction: number) => void;
  ended?: (reason: "timer" | "held") => void;
  /** The panel asked the main window to show the wipe-down card. */
  open?: () => void;
}): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return Promise.all([
    listen<{ seconds: number }>("wipe:started", (e) => handlers.started?.(e.payload.seconds)),
    listen<{ fraction: number }>("wipe:held", (e) => handlers.held?.(e.payload.fraction)),
    listen<{ reason: "timer" | "held" }>("wipe:ended", (e) =>
      handlers.ended?.(e.payload.reason),
    ),
    listen("wipe:open", () => handlers.open?.()),
  ]).then((fns) => () => fns.forEach((fn) => fn()));
}

/** Ask the main window to show the wipe-down card (used by the panel). */
export function requestWipe(): Promise<void> {
  if (!engineAvailable()) return Promise.resolve();
  return emit("wipe:open");
}

/** Subscribe to duplicate-search progress. */
export function onDuplicateProgress(
  handler: (progress: DuplicateProgress) => void,
): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<DuplicateProgress>(EVENT_DUPLICATE_PROGRESS, (event) => handler(event.payload));
}

/** Subscribe to the end of a duplicate search. */
export function onDuplicateFinished(
  handler: (stats: DuplicateStats) => void,
): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<DuplicateStats>(EVENT_DUPLICATE_FINISHED, (event) => handler(event.payload));
}

export function onDuplicateFailed(handler: (message: string) => void): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<string>(EVENT_DUPLICATE_FAILED, (event) => handler(event.payload));
}

/** Subscribe to coalesced scan progress. Returns an unsubscribe function. */
export function onScanProgress(handler: (progress: ScanProgress) => void): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<ScanProgress>(EVENT_SCAN_PROGRESS, (event) => handler(event.payload));
}

/** Subscribe to the terminal event of a scan. */
export function onScanFinished(handler: (result: ScanFinished) => void): Promise<UnlistenFn> {
  if (!engineAvailable()) return Promise.resolve(() => {});
  return listen<ScanFinished>(EVENT_SCAN_FINISHED, (event) => handler(event.payload));
}

/** Turn any thrown value into a message worth showing. */
export function describeError(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return "Something went wrong.";
}
