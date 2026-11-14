import { useCallback, useEffect, useState } from "react";

import { Note } from "../components/primitives";
import { ApplicationsPage } from "../features/applications/ApplicationsPage";
import { CleanupPage } from "../features/cleanup/CleanupPage";
import { QueueBar, QueueDrawer } from "../features/cleanup/Queue";
import { useCleanup } from "../features/cleanup/useCleanup";
import { MemoryPage } from "../features/memory/MemoryPage";
import { OverviewPage } from "../features/overview/OverviewPage";
import { StoragePage } from "../features/storage/StoragePage";
import { WipeOverlay, WipeSheet } from "../features/wipe/Wipe";
import { engine, engineAvailable, onScanFinished, onScanProgress, onWipe } from "../lib/ipc";
import type { AppInfo, MemoryPressure, ScanSummary, StartupReport } from "../lib/types";
import type { AppsTab } from "../features/applications/ApplicationsPage";

import { TopNav, type Destination, type SectionId } from "./TopNav";

/** Sections whose screens can add to the cleanup queue. */
const QUEUE_SECTIONS: ReadonlySet<SectionId> = new Set(["cleanup", "applications"]);

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
  /** Which tab Apps opens on; Overview's "Starts with your Mac" opens Startup. */
  const [appsTab, setAppsTab] = useState<AppsTab>("uninstall");
  /** The wipe-down sheet; the menu bar panel can open it too. */
  const [wipeOpen, setWipeOpen] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onWipe({ open: () => setWipeOpen(true) }).then((fn) => (unlisten = fn));
    return () => unlisten?.();
  }, []);

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

  // Listened to here, not in Storage, so the top bar can say a scan is running
  // while you look at other sections.
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

  /** Cleanup shows its findings, or the duplicates view opened from them. */
  const [showDuplicates, setShowDuplicates] = useState(false);

  const go = useCallback((next: Destination) => {
    setSection(next === "duplicates" ? "cleanup" : next);
    setShowDuplicates(next === "duplicates");
    setAppsTab("uninstall");
    setQueueOpen(false);
  }, []);

  const goToStartup = useCallback(() => {
    setSection("applications");
    setAppsTab("startup");
    setQueueOpen(false);
  }, []);

  const volumeName = scan
    ? scan.rootPath === "/"
      ? "Startup disk"
      : (scan.rootPath.replace(/\/+$/, "").split("/").pop() ?? scan.rootPath)
    : null;

  return (
    <div className="shell" data-section={section}>
      <TopNav
        active={section}
        onSelect={go}
        volumeName={volumeName}
        groups={cleanup.groups}
        stagedCount={cleanup.staged.length}
        stagedBytes={cleanup.stagedTotal}
        scanning={scanning}
        pressure={pressure}
      />

      <main
        className={[
          "main",
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
              scan={scan}
              scanning={scanning}
              cleanup={cleanup}
              startup={startup}
              onPressure={setPressure}
              onGo={go}
              onGoToStartup={goToStartup}
              onWipe={() => setWipeOpen(true)}
            />
          )}
          {section === "storage" && <StoragePage onScanReady={onScanReady} />}
          {section === "cleanup" && (
            <CleanupPage
              scanId={scan?.scanId ?? null}
              totalFiles={scan?.fileCount ?? null}
              cleanup={cleanup}
              showDuplicates={showDuplicates}
              onShowDuplicates={setShowDuplicates}
              onGoToStorage={() => go("storage")}
            />
          )}
          {section === "applications" && (
            <ApplicationsPage
              cleanup={cleanup}
              tab={appsTab}
              onTab={setAppsTab}
              startup={startup}
              onStartup={setStartup}
            />
          )}
          {section === "memory" && <MemoryPage onPressure={setPressure} />}
        </div>

        {QUEUE_SECTIONS.has(section) && (
          <QueueBar cleanup={cleanup} onOpen={() => setQueueOpen(true)} />
        )}
      </main>

      <QueueDrawer cleanup={cleanup} open={queueOpen} onClose={() => setQueueOpen(false)} />
      <WipeSheet open={wipeOpen} onClose={() => setWipeOpen(false)} />
      <WipeOverlay onAgain={() => setWipeOpen(true)} />
    </div>
  );
}
