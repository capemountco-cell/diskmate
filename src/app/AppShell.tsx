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
import { engine, engineAvailable } from "../lib/ipc";
import type { AppInfo, ScanSummary, StartupReport } from "../lib/types";

import { Sidebar, type SectionId } from "./Sidebar";

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
  const [storageIntro, setStorageIntro] = useState(false);
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

  const onScanReady = useCallback((next: ScanSummary | null) => {
    // Storage reports null while it is still finding its index; that should
    // not erase a scan the shell already knows about.
    if (next) setScan(next);
  }, []);

  const go = useCallback((next: SectionId) => {
    setSection(next);
    setQueueOpen(false);
    if (next !== "storage") setStorageIntro(false);
  }, []);

  // The sidebar carries labels while a section is introducing itself.
  const expanded =
    (section === "storage" && storageIntro) || (section === "startup" && startup === null);

  const marked = useMemo(() => {
    const set = new Set<SectionId>();
    if (cleanup.groups.length > 0) set.add("cleanup");
    if (cleanup.duplicates.length > 0) set.add("duplicates");
    if (cleanup.staged.length > 0) set.add("cleanup");
    return set;
  }, [cleanup.groups.length, cleanup.duplicates.length, cleanup.staged.length]);

  return (
    <div className="shell" data-section={section}>
      <Sidebar active={section} onSelect={go} info={info} expanded={expanded} marked={marked} />

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
                {info.portableFallbackReason} DiskMate is storing its index on this Mac instead,
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
          {section === "storage" && (
            <StoragePage onScanReady={onScanReady} onIntro={setStorageIntro} />
          )}
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
          {section === "memory" && <MemoryPage />}
        </div>

        {QUEUE_SECTIONS.has(section) && (
          <QueueBar cleanup={cleanup} onOpen={() => setQueueOpen(true)} />
        )}
      </main>

      <QueueDrawer cleanup={cleanup} open={queueOpen} onClose={() => setQueueOpen(false)} />
    </div>
  );
}
