import { useCallback, useEffect, useMemo, useState } from "react";

import { Note } from "../components/primitives";
import { ApplicationsPage } from "../features/applications/ApplicationsPage";
import { CleanupPage } from "../features/cleanup/CleanupPage";
import { QueueBar, QueueDrawer } from "../features/cleanup/Queue";
import { useCleanup } from "../features/cleanup/useCleanup";
import { DuplicatesPage } from "../features/duplicates/DuplicatesPage";
import { MemoryPage } from "../features/memory/MemoryPage";
import { OverviewPage } from "../features/overview/OverviewPage";
import { StartupPage } from "../features/startup/StartupPage";
import { StoragePage } from "../features/storage/StoragePage";
import { engine, engineAvailable, onScanFinished, onScanProgress } from "../lib/ipc";
import type { AppInfo, MemoryPressure, ScanSummary, StartupReport } from "../lib/types";

import { TopNav, type SectionId } from "./TopNav";

/** Sections whose screens can add to the cleanup queue. */
const QUEUE_SECTIONS: ReadonlySet<SectionId> = new Set([
  "cleanup",
  "duplicates",
  "applications",
]);

export function AppShell() {
  const [section, setSection] = useState<SectionId>("overview");
  const [info, setInfo] = useState<AppInfo | null>(null);
  /**
   * The scan Cleanup and the visualisations work from.
   *
   * Lifted here rather than kept in Storage because Cleanup reads from the same
   * scan, and the alternative - each section scanning for itself - would make
   * the app do the work twice and possibly disagree with itself.
   */
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [scanning, setScanning] = useState(false);
  const [pressure, setPressure] = useState<MemoryPressure>("unknown");
  const [startup, setStartup] = useState<StartupReport | null>(null);
  const [queueOpen, setQueueOpen] = useState(false);

  /**
   * One cleanup controller for the whole app.
   *
   * Cleanup, Duplicates and Applications all add to the same queue, so they
   * share the state that holds it rather than each keeping a copy.
   */
  const cleanup = useCleanup(scan?.scanId ?? null);

  useEffect(() => {
    if (!engineAvailable()) return;
    engine
      .appInfo()
      .then(setInfo)
      .catch(() => setInfo(null));
    // Adopt the most recent scan on launch, so This Mac and Cleanup have
    // something to show before Storage has been opened.
    engine
      .recentScans(5)
      .then((scans) =>
        setScan(
          (current) =>
            current ?? scans.find((candidate) => candidate.status === "completed") ?? null,
        ),
      )
      .catch(() => undefined);
  }, []);

  // A scan is the main thing that turns the window amber. Listened to here,
  // not in Storage, so the colour holds while you look at other sections.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    void onScanProgress(() => setScanning(true)).then((fn) => unlisteners.push(fn));
    void onScanFinished(() => setScanning(false)).then((fn) => unlisteners.push(fn));
    return () => unlisteners.forEach((fn) => fn());
  }, []);

  const onScanReady = useCallback((next: ScanSummary | null) => {
    // Storage reports null while it is still finding its index; that should
    // not erase a scan the shell already knows about.
    if (next) setScan(next);
  }, []);

  const go = useCallback((next: SectionId) => {
    setSection(next);
    setQueueOpen(false);
  }, []);

  /**
   * What is true right now, which is the only thing that colours the window.
   *
   * Black and white until something is true; then one colour, meaning one
   * thing, in this order of precedence:
   *   working  amber  a scan or a duplicate search is running
   *   high     red    memory pressure is high (Memory only)
   *   done     green  the queue was just moved to the Trash
   *   found    teal   Cleanup or Duplicates has space to show you
   *   calm     green  memory pressure is normal (Memory only)
   */
  const state = useMemo(() => {
    if (scanning || cleanup.duplicateProgress) return "working";
    if (section === "memory" && pressure === "critical") return "high";
    if (section === "memory" && pressure === "warning") return "working";
    if (cleanup.report && cleanup.report.removedCount > 0) return "done";
    if (section === "cleanup" && cleanup.groups.length > 0) return "found";
    if (section === "duplicates" && cleanup.duplicates.length > 0) return "copies";
    if (section === "memory" && pressure === "normal") return "calm";
    return "idle";
  }, [
    scanning,
    cleanup.duplicateProgress,
    cleanup.report,
    cleanup.groups.length,
    cleanup.duplicates.length,
    section,
    pressure,
  ]);

  const volumeName = scan
    ? scan.rootPath === "/"
      ? "Startup disk"
      : (scan.rootPath.replace(/\/+$/, "").split("/").pop() ?? scan.rootPath)
    : null;

  return (
    <div className="shell" data-section={section} data-state={state}>
      <TopNav
        active={section}
        onSelect={go}
        volumeName={volumeName}
        groups={cleanup.groups}
        stagedCount={cleanup.staged.length}
        stagedBytes={cleanup.stagedTotal}
      />

      <main
        className={[
          "main",
          section === "applications" ? "main--framed" : "",
          QUEUE_SECTIONS.has(section) && cleanup.staged.length > 0 ? "main--queue" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        <div className="main__body">
          {info?.portableFallbackReason && (
            <div style={{ marginBottom: "var(--space-4)" }}>
              <Note tone="caution" title="Portable mode is not active.">
                {info.portableFallbackReason} Mr Clean is storing its index on this Mac instead,
                at {info.dataDir}.
              </Note>
            </div>
          )}

          {section === "overview" && (
            <OverviewPage
              info={info}
              scan={scan}
              groups={cleanup.groups}
              startup={startup}
              onGo={go}
            />
          )}
          {section === "storage" && <StoragePage onScanReady={onScanReady} />}
          {section === "cleanup" && (
            <CleanupPage
              scanId={scan?.scanId ?? null}
              cleanup={cleanup}
              onGoToStorage={() => go("storage")}
            />
          )}
          {section === "duplicates" && (
            <DuplicatesPage
              scanId={scan?.scanId ?? null}
              totalFiles={scan?.fileCount ?? null}
              cleanup={cleanup}
              onGoToStorage={() => go("storage")}
            />
          )}
          {section === "applications" && (
            <ApplicationsPage cleanup={cleanup} onBack={() => go("cleanup")} />
          )}
          {section === "startup" && <StartupPage report={startup} onReport={setStartup} />}
          {section === "memory" && <MemoryPage onPressure={setPressure} />}
        </div>

        {QUEUE_SECTIONS.has(section) && (
          <QueueBar cleanup={cleanup} onOpen={() => setQueueOpen(true)} />
        )}
      </main>

      <QueueDrawer cleanup={cleanup} open={queueOpen} onClose={() => setQueueOpen(false)} />
    </div>
  );
}
