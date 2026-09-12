/**
 * Cleanup state.
 *
 * The queue is the centre of this screen and the only route to removal. It is
 * held here so that staging, reviewing and approving are one flow with one
 * source of truth, rather than three components each with their own idea of
 * what is about to be deleted.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  describeError,
  engine,
  engineAvailable,
  onDuplicateFailed,
  onDuplicateFinished,
  onDuplicateProgress,
} from "../../lib/ipc";
import type {
  Application,
  CandidateGroup,
  CleanupCandidate,
  DuplicateGroup,
  DuplicateProgress,
  DuplicateStats,
  RemovalReport,
} from "../../lib/types";

export type CleanupTab = "suggestions" | "duplicates" | "applications";

export function useCleanup(scanId: number | null) {
  const [tab, setTab] = useState<CleanupTab>("suggestions");
  const [groups, setGroups] = useState<CandidateGroup[]>([]);
  const [candidates, setCandidates] = useState<CleanupCandidate[]>([]);
  const [staged, setStaged] = useState<CleanupCandidate[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<RemovalReport | null>(null);

  const [duplicates, setDuplicates] = useState<DuplicateGroup[]>([]);
  const [duplicateProgress, setDuplicateProgress] = useState<DuplicateProgress | null>(null);
  const [duplicateStats, setDuplicateStats] = useState<DuplicateStats | null>(null);

  const [applications, setApplications] = useState<Application[]>([]);
  const [loadingApps, setLoadingApps] = useState(false);

  const refreshQueue = useCallback(async () => {
    try {
      setStaged(await engine.stagedCleanup());
    } catch (cause) {
      setError(describeError(cause));
    }
  }, []);

  const refresh = useCallback(async () => {
    if (scanId === null || !engineAvailable()) return;
    setBusy(true);
    try {
      await engine.proposeCleanup(scanId);
      const [groupRows, candidateRows] = await Promise.all([
        engine.cleanupGroups(scanId),
        engine.cleanupCandidates(scanId, null, 500),
      ]);
      setGroups(groupRows);
      setCandidates(candidateRows);
      await refreshQueue();
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }, [scanId, refreshQueue]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Duplicate search reports through events, because hashing outlives any
  // single command call.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    void onDuplicateProgress(setDuplicateProgress).then((fn) => unlisteners.push(fn));
    void onDuplicateFinished(async (stats) => {
      setDuplicateStats(stats);
      setDuplicateProgress(null);
      if (scanId !== null) {
        try {
          setDuplicates(await engine.duplicateGroups(scanId, 100));
          setCandidates(await engine.cleanupCandidates(scanId, null, 500));
          setGroups(await engine.cleanupGroups(scanId));
        } catch (cause) {
          setError(describeError(cause));
        }
      }
    }).then((fn) => unlisteners.push(fn));
    void onDuplicateFailed((message) => {
      setError(message);
      setDuplicateProgress(null);
    }).then((fn) => unlisteners.push(fn));
    return () => unlisteners.forEach((fn) => fn());
  }, [scanId]);

  const findDuplicates = useCallback(async () => {
    if (scanId === null) return;
    setDuplicateStats(null);
    setDuplicateProgress({ stage: "starting", done: 0, total: 0 });
    try {
      await engine.startDuplicateSearch(scanId);
    } catch (cause) {
      setError(describeError(cause));
      setDuplicateProgress(null);
    }
  }, [scanId]);

  const loadApplications = useCallback(async () => {
    setLoadingApps(true);
    try {
      setApplications(await engine.inspectApplications());
      setError(null);
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setLoadingApps(false);
    }
  }, []);

  const stage = useCallback(
    async (ids: number[]) => {
      try {
        await engine.stageCleanup(ids);
        await refreshQueue();
      } catch (cause) {
        setError(describeError(cause));
      }
    },
    [refreshQueue],
  );

  const unstage = useCallback(
    async (ids: number[]) => {
      try {
        await engine.unstageCleanup(ids);
        await refreshQueue();
      } catch (cause) {
        setError(describeError(cause));
      }
    },
    [refreshQueue],
  );

  const uninstall = useCallback(
    async (applicationId: number) => {
      try {
        const proposed = await engine.proposeUninstall(applicationId);
        // Stage only what is safe to stage; anything protected is refused by
        // the engine anyway, and asking it to try would be noise.
        await engine.stageCleanup(
          proposed.filter((c) => c.safety !== "protected").map((c) => c.id),
        );
        await refreshQueue();
      } catch (cause) {
        setError(describeError(cause));
      }
    },
    [refreshQueue],
  );

  const stagedTotal = useMemo(
    () => staged.reduce((sum, candidate) => sum + candidate.bytes, 0),
    [staged],
  );

  /**
   * Approve the queue exactly as it is shown.
   *
   * The ids and the total sent here are the ones on screen. If the queue has
   * changed underneath, the engine refuses the whole operation rather than
   * removing something nobody reviewed.
   */
  const removeApproved = useCallback(async () => {
    if (staged.length === 0) return;
    setBusy(true);
    try {
      const result = await engine.removeApproved({
        candidateIds: staged.map((candidate) => candidate.id),
        expectedTotalBytes: stagedTotal,
      });
      setReport(result);
      setError(null);
      await refresh();
    } catch (cause) {
      setError(describeError(cause));
    } finally {
      setBusy(false);
    }
  }, [staged, stagedTotal, refresh]);

  const stagedIds = useMemo(() => new Set(staged.map((c) => c.id)), [staged]);

  return {
    tab,
    setTab,
    groups,
    candidates,
    staged,
    stagedIds,
    stagedTotal,
    duplicates,
    duplicateProgress,
    duplicateStats,
    applications,
    loadingApps,
    error,
    busy,
    report,
    dismissReport: () => setReport(null),
    refresh,
    stage,
    unstage,
    findDuplicates,
    loadApplications,
    uninstall,
    removeApproved,
  };
}
