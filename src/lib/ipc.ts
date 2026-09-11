/**
 * Typed access to the engine.
 *
 * Every call the UI makes to Rust goes through this module. Components never
 * call `invoke` directly, so the command surface stays visible in one place and
 * the argument shapes cannot drift apart across features.
 *
 * When DiskMate's window is opened by Vite alone (`npm run dev:web`), the Tauri
 * bridge is absent. That case reports itself plainly rather than substituting
 * sample data: a storage tool showing invented numbers would be worse than one
 * showing none.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppInfo,
  ChildPage,
  ChildQuery,
  EntryRow,
  MemoryReport,
  ScanFinished,
  ScanProgress,
  ScanSummary,
  StartupReport,
  StorageSnapshot,
  VolumeInfo,
} from "./types";

export const EVENT_SCAN_PROGRESS = "diskmate://scan-progress";
export const EVENT_SCAN_FINISHED = "diskmate://scan-finished";

/** True when the app is running inside its Tauri window. */
export function engineAvailable(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export class EngineUnavailableError extends Error {
  constructor() {
    super(
      "DiskMate's engine is not running. Start the app with `npm run dev` rather than " +
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
  startupReport: () => call<StartupReport>("startup_report"),
};

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
