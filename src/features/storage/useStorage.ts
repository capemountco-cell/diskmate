/**
 * Storage screen state.
 *
 * A hand-written reducer rather than a state library: the screen has one
 * lifecycle (choose a volume, scan or reuse an index, browse it) and a handful
 * of transitions, and making them explicit here is what keeps components free
 * of logic.
 *
 * The UI never holds a filesystem tree. It holds the current directory's id and
 * one page of its children; every navigation is a fresh bounded query.
 */

import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";

import {
  describeError,
  engine,
  engineAvailable,
  onScanFinished,
  onScanProgress,
} from "../../lib/ipc";
import type {
  EntryRow,
  ScanProgress,
  ScanSummary,
  SortDirection,
  SortKey,
  StorageSnapshot,
  VolumeInfo,
} from "../../lib/types";

const PAGE_SIZE = 200;

export type ScanPhase = "idle" | "scanning" | "ready" | "failed";

interface State {
  volumes: VolumeInfo[];
  /**
   * The directory a scan applies to. Usually a volume's mount point, but the
   * user can also pick any folder: on a Mac without Full Disk Access that is
   * the difference between a useful app and a blocked one, because macOS grants
   * access to a folder the user chose in a panel even when it withholds the
   * disk as a whole.
   */
  selectedRoot: string | null;
  volumesError: string | null;
  loadingVolumes: boolean;

  phase: ScanPhase;
  progress: ScanProgress | null;
  snapshot: StorageSnapshot | null;
  scanError: string | null;

  /** Directory currently open in the explorer. */
  currentDirectory: EntryRow | null;
  ancestors: EntryRow[];
  children: EntryRow[];
  childTotal: number;
  loadingChildren: boolean;

  sort: SortKey;
  direction: SortDirection;
  hideHidden: boolean;
}

type Action =
  | { type: "volumes/loading" }
  | { type: "volumes/loaded"; volumes: VolumeInfo[] }
  | { type: "volumes/failed"; message: string }
  | { type: "root/select"; root: string }
  | { type: "scan/started" }
  | { type: "scan/progress"; progress: ScanProgress }
  | { type: "scan/failed"; message: string }
  | { type: "scan/ready"; snapshot: StorageSnapshot }
  | { type: "scan/none" }
  | { type: "children/loading" }
  | { type: "children/loaded"; directory: EntryRow; ancestors: EntryRow[]; rows: EntryRow[]; total: number }
  | { type: "sort/set"; sort: SortKey; direction: SortDirection }
  | { type: "hidden/toggle" };

const initialState: State = {
  volumes: [],
  selectedRoot: null,
  volumesError: null,
  loadingVolumes: true,
  phase: "idle",
  progress: null,
  snapshot: null,
  scanError: null,
  currentDirectory: null,
  ancestors: [],
  children: [],
  childTotal: 0,
  loadingChildren: false,
  sort: "size",
  direction: "desc",
  hideHidden: false,
};

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case "volumes/loading":
      return { ...state, loadingVolumes: true, volumesError: null };
    case "volumes/loaded": {
      // Prefer the boot volume, which is what a person almost always means.
      const preferred =
        state.selectedRoot ??
        action.volumes.find((volume) => volume.mountPoint === "/")?.mountPoint ??
        action.volumes[0]?.mountPoint ??
        null;
      return { ...state, volumes: action.volumes, selectedRoot: preferred, loadingVolumes: false };
    }
    case "volumes/failed":
      return { ...state, loadingVolumes: false, volumesError: action.message };
    case "root/select":
      // Changing the target resets everything derived from the old index, but
      // keeps the volume list and the user's view preferences.
      return {
        ...initialState,
        volumes: state.volumes,
        loadingVolumes: false,
        sort: state.sort,
        direction: state.direction,
        hideHidden: state.hideHidden,
        selectedRoot: action.root,
      };
    case "scan/started":
      return { ...state, phase: "scanning", progress: null, scanError: null };
    case "scan/progress":
      return { ...state, phase: "scanning", progress: action.progress };
    case "scan/failed":
      return { ...state, phase: "failed", scanError: action.message, progress: null };
    case "scan/ready":
      return {
        ...state,
        phase: "ready",
        snapshot: action.snapshot,
        progress: null,
        scanError: null,
        currentDirectory: action.snapshot.root,
        ancestors: [action.snapshot.root],
      };
    case "scan/none":
      return { ...state, phase: "idle", snapshot: null, progress: null, currentDirectory: null };
    case "children/loading":
      return { ...state, loadingChildren: true };
    case "children/loaded":
      return {
        ...state,
        loadingChildren: false,
        currentDirectory: action.directory,
        ancestors: action.ancestors,
        children: action.rows,
        childTotal: action.total,
      };
    case "sort/set":
      return { ...state, sort: action.sort, direction: action.direction };
    case "hidden/toggle":
      return { ...state, hideHidden: !state.hideHidden };
    default:
      return state;
  }
}

export function useStorage() {
  const [state, dispatch] = useReducer(reducer, initialState);
  // Guards a late response from a directory the user has already navigated
  // away from, which otherwise repaints the table with stale rows.
  const navigationToken = useRef(0);

  const refreshVolumes = useCallback(async () => {
    dispatch({ type: "volumes/loading" });
    try {
      dispatch({ type: "volumes/loaded", volumes: await engine.listVolumes() });
    } catch (error) {
      dispatch({ type: "volumes/failed", message: describeError(error) });
    }
  }, []);

  useEffect(() => {
    void refreshVolumes();
  }, [refreshVolumes]);

  const loadSnapshot = useCallback(async (scanId: number) => {
    try {
      dispatch({ type: "scan/ready", snapshot: await engine.storageSnapshot(scanId) });
    } catch (error) {
      dispatch({ type: "scan/failed", message: describeError(error) });
    }
  }, []);

  // Adopt an existing index for the selected volume so reopening the app does
  // not force a rescan.
  useEffect(() => {
    if (!state.selectedRoot || !engineAvailable()) return;
    let cancelled = false;
    void (async () => {
      try {
        const latest = await engine.latestScan(state.selectedRoot!);
        if (cancelled) return;
        if (latest) await loadSnapshot(latest.scanId);
        else dispatch({ type: "scan/none" });
      } catch (error) {
        if (!cancelled) dispatch({ type: "scan/failed", message: describeError(error) });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [state.selectedRoot, loadSnapshot]);

  // Scan progress and completion arrive as events, not as a promise, because a
  // scan outlives any single command call.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    void onScanProgress((progress) => dispatch({ type: "scan/progress", progress })).then((fn) =>
      unlisteners.push(fn),
    );
    void onScanFinished((result) => {
      if (result.summary) void loadSnapshot(result.summary.scanId);
      else dispatch({ type: "scan/failed", message: result.error ?? "The scan did not finish." });
    }).then((fn) => unlisteners.push(fn));
    return () => unlisteners.forEach((fn) => fn());
  }, [loadSnapshot]);

  const startScan = useCallback(async () => {
    if (!state.selectedRoot) return;
    dispatch({ type: "scan/started" });
    try {
      await engine.startScan(state.selectedRoot);
    } catch (error) {
      dispatch({ type: "scan/failed", message: describeError(error) });
    }
  }, [state.selectedRoot]);

  const cancelScan = useCallback(async () => {
    try {
      await engine.cancelScan();
    } catch {
      // Cancelling something that already stopped is not worth reporting.
    }
  }, []);

  const openDirectory = useCallback(
    async (entry: EntryRow) => {
      const scanId = state.snapshot?.scan.scanId;
      if (!scanId || entry.kind !== "directory") return;
      const token = (navigationToken.current += 1);
      dispatch({ type: "children/loading" });
      try {
        const [page, ancestors] = await Promise.all([
          engine.listChildren({
            scanId,
            parentId: entry.id,
            sort: state.sort,
            direction: state.direction,
            limit: PAGE_SIZE,
            offset: 0,
            hideHidden: state.hideHidden,
          }),
          engine.entryAncestors(scanId, entry.id),
        ]);
        if (token !== navigationToken.current) return;
        dispatch({
          type: "children/loaded",
          directory: entry,
          ancestors,
          rows: page.rows,
          total: page.total,
        });
      } catch (error) {
        if (token === navigationToken.current) {
          dispatch({ type: "scan/failed", message: describeError(error) });
        }
      }
    },
    [state.snapshot, state.sort, state.direction, state.hideHidden],
  );

  // Re-run the current listing whenever sorting or the hidden filter changes.
  const currentId = state.currentDirectory?.id;
  useEffect(() => {
    if (!state.currentDirectory) return;
    void openDirectory(state.currentDirectory);
    // `openDirectory` closes over the sort state, so depending on the id alone
    // would miss a sort change; depending on `openDirectory` itself is what
    // makes this re-run exactly when the query shape changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentId, state.sort, state.direction, state.hideHidden]);

  const setSort = useCallback(
    (key: SortKey) => {
      dispatch({
        type: "sort/set",
        sort: key,
        // Sizes and counts are most useful largest-first; names are not.
        direction:
          state.sort === key
            ? state.direction === "asc"
              ? "desc"
              : "asc"
            : key === "name"
              ? "asc"
              : "desc",
      });
    },
    [state.sort, state.direction],
  );

  /** Null when the chosen root is a folder rather than a whole volume. */
  const selectedVolume = useMemo(
    () => state.volumes.find((volume) => volume.mountPoint === state.selectedRoot) ?? null,
    [state.volumes, state.selectedRoot],
  );

  const chooseFolder = useCallback(async () => {
    try {
      const chosen = await open({ directory: true, multiple: false, title: "Choose a folder to scan" });
      if (typeof chosen === "string") dispatch({ type: "root/select", root: chosen });
    } catch (error) {
      dispatch({ type: "scan/failed", message: describeError(error) });
    }
  }, []);

  return {
    ...state,
    selectedVolume,
    refreshVolumes,
    selectRoot: (root: string) => dispatch({ type: "root/select", root }),
    chooseFolder,
    startScan,
    cancelScan,
    openDirectory,
    setSort,
    toggleHidden: () => dispatch({ type: "hidden/toggle" }),
  };
}

export type StorageController = ReturnType<typeof useStorage>;
export type { ScanSummary };
