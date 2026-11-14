import { useEffect, useState } from "react";

import { MapIcon, RankIcon, StorageIcon, TrendIcon } from "../../components/icons";
import { Hero, IntroScreen, PageHeader } from "../../components/layout";
import { Button, EmptyState, Note, Panel } from "../../components/primitives";
import { formatCount } from "../../lib/format";
import { engineAvailable } from "../../lib/ipc";
import type { EntryRow } from "../../lib/types";

import { Breadcrumbs } from "./Breadcrumbs";
import { Inspector } from "./Inspector";
import { EntryTable } from "./EntryTable";
import { ScanStatusBar } from "./ScanStatusBar";
import { SnapshotPanel } from "./SnapshotPanel";
import { StorageOverview } from "./StorageOverview";
import { StorageViews } from "./StorageViews";
import { CapacitySummary, VolumePicker } from "./VolumePicker";
import { useStorage, type ScanSummary } from "./useStorage";

export function StoragePage({
  onScanReady,
}: {
  onScanReady: (scan: ScanSummary | null) => void;
}) {
  const storage = useStorage();
  const currentScan = storage.snapshot?.scan ?? null;
  // What the inspector is describing. Null means "whatever folder we are in",
  // so the panel is never empty and never has to be dismissed.
  const [selected, setSelected] = useState<EntryRow | null>(null);

  // Tell the shell which scan is current, so Cleanup works from the same one
  // rather than starting its own.
  useEffect(() => {
    onScanReady(currentScan);
  }, [currentScan, onScanReady]);
  const { snapshot, phase, selectedVolume, currentDirectory, ancestors, children, childTotal } =
    storage;
  const intro =
    engineAvailable() &&
    !storage.loadingVolumes &&
    !snapshot &&
    (phase === "idle" || phase === "failed");

  const open = (row: EntryRow) => {
    setSelected(null);
    void storage.openDirectory(row);
  };
  const select = (row: EntryRow) => setSelected(row);

  if (!engineAvailable()) {
    return (
      <EmptyState title="The Mr Clean engine is not running">
        This window is showing the web build on its own. Start the application with{" "}
        <code className="mono">npm run dev</code> so the scanner and index are available.
      </EmptyState>
    );
  }

  if (intro) return <StorageIntro storage={storage} />;

  const atRoot = currentDirectory !== null && currentDirectory.id === snapshot?.root.id;
  const inspecting = selected ?? currentDirectory;

  return (
    <div className="page">
      <PageHeader title="Storage" />
      {phase === "scanning" && (
        <Hero
          title={`Reading ${targetName(storage)}…`}
          subtitle="Mr Clean reads file and folder sizes. It does not open, move, change or delete anything."
        />
      )}
      <div className="stack">
        <VolumePicker
          volumes={storage.volumes}
          selectedRoot={storage.selectedRoot}
          loading={storage.loadingVolumes}
          error={storage.volumesError}
          onSelect={storage.selectRoot}
          onRefresh={storage.refreshVolumes}
          onChooseFolder={storage.chooseFolder}
        />

        {selectedVolume && !selectedVolume.isReadable && (
          <Note tone="caution" title="Mr Clean cannot read this volume.">
            macOS has not granted access to it. On the boot volume this usually means Full Disk
            Access is off; you can turn it on in System Settings under Privacy &amp; Security.
          </Note>
        )}

        {selectedVolume && (
          <CapacitySummary
            volume={selectedVolume}
            indexedBytes={snapshot ? snapshot.scan.logicalBytes : null}
          />
        )}

        {phase === "scanning" && (
          <ScanStatusBar progress={storage.progress} onCancel={storage.cancelScan} />
        )}

        {phase === "failed" && storage.scanError && (
          <Note tone="caution" title="The scan did not finish.">
            {storage.scanError}
          </Note>
        )}

        {snapshot && phase !== "scanning" && (
          <>
            <div className="row row--between">
              <Breadcrumbs trail={ancestors} onNavigate={storage.openDirectory} />
              <div className="row">
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={storage.hideHidden}
                    onChange={storage.toggleHidden}
                  />
                  Hide hidden items
                </label>
                <Button onClick={storage.startScan}>Scan again</Button>
              </div>
            </div>

            <div className="workspace">
              <div className="stack workspace__main">
                {atRoot ? (
                  <>
                    <StorageOverview snapshot={snapshot} onOpen={select} />
                    <StorageViews
                      scanId={snapshot.scan.scanId}
                      root={snapshot.root}
                      onOpen={open}
                      selectedId={inspecting?.id ?? null}
                      onSelect={setSelected}
                      explorer={
                        <ExplorerBody
                          storage={storage}
                          rows={children}
                          total={childTotal}
                          onSelect={setSelected}
                        />
                      }
                    />
                    <SnapshotPanel scanId={snapshot.scan.scanId} />
                  </>
                ) : (
                  <Panel
                    title={currentDirectory?.name}
                    note={currentDirectory ? `${formatCount(childTotal)} items` : undefined}
                    flush
                  >
                    <ExplorerBody
                      storage={storage}
                      rows={children}
                      total={childTotal}
                      onSelect={setSelected}
                    />
                  </Panel>
                )}
              </div>

              <Inspector
                scanId={snapshot.scan.scanId}
                entry={inspecting}
                scanBytes={snapshot.scan.logicalBytes}
                onOpen={open}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function targetName(storage: ReturnType<typeof useStorage>): string {
  if (storage.selectedVolume) return storage.selectedVolume.name;
  const root = storage.selectedRoot ?? "";
  const cut = root.replace(/\/+$/, "").lastIndexOf("/");
  return cut >= 0 ? root.slice(cut + 1) || root : root;
}

function ExplorerBody({
  storage,
  rows,
  total,
  onSelect,
}: {
  storage: ReturnType<typeof useStorage>;
  rows: EntryRow[];
  total: number;
  onSelect: (row: EntryRow) => void;
}) {
  const directory = storage.currentDirectory;
  if (directory && !directory.accessible) {
    return (
      <p className="muted" style={{ padding: "var(--space-5)" }}>
        Mr Clean could not read inside this folder, so its contents are not in the index. This
        is not the same as the folder being empty.
      </p>
    );
  }
  return (
    <EntryTable
      rows={rows}
      total={total}
      sort={storage.sort}
      direction={storage.direction}
      onSort={storage.setSort}
      onOpen={storage.openDirectory}
      onSelect={onSelect}
      emptyMessage="This folder is empty."
    />
  );
}

/**
 * Storage before anything has been scanned.
 *
 * The volume choice sits under the copy so the one button below always says
 * what it will read.
 */
function StorageIntro({ storage }: { storage: ReturnType<typeof useStorage> }) {
  const target = storage.selectedRoot ? targetName(storage) : null;
  return (
    <>
      <IntroScreen
        title="Storage"
        subtitle="See where your space went, folder by folder, and what changed since you last looked."
        art={
          <div className="device-art" aria-hidden="true">
            <StorageIcon size={72} />
          </div>
        }
        features={[
          { icon: <MapIcon size={20} />, label: "Map of Every Folder" },
          { icon: <RankIcon size={20} />, label: "Largest Files, Ranked" },
          { icon: <TrendIcon size={20} />, label: "Changes Since Last Look" },
        ]}
        action={{
          label: "Scan",
          onClick: storage.startScan,
          disabled: !storage.selectedRoot || storage.selectedVolume?.isReadable === false,
        }}
        footnote={target ? `${target} · reads only, changes nothing` : "Choose what to scan"}
      >
        <div className="intro__choices" role="group" aria-label="What to scan">
          {storage.volumes.map((volume) => (
            <button
              key={volume.mountPoint}
              type="button"
              className="chip-button"
              aria-pressed={volume.mountPoint === storage.selectedRoot}
              onClick={() => storage.selectRoot(volume.mountPoint)}
              title={volume.mountPoint}
            >
              {volume.name}
            </button>
          ))}
          {storage.selectedRoot &&
            !storage.volumes.some((v) => v.mountPoint === storage.selectedRoot) && (
              <button type="button" className="chip-button" aria-pressed>
                {targetName(storage)}
              </button>
            )}
          <button type="button" className="chip-button" onClick={storage.chooseFolder}>
            Choose a Folder…
          </button>
        </div>
      </IntroScreen>
      {(storage.volumesError ||
        storage.scanError ||
        (storage.selectedVolume && !storage.selectedVolume.isReadable)) && (
        <div className="intro__error">
          {storage.volumesError && <Note tone="caution">{storage.volumesError}</Note>}
          {storage.scanError && (
            <Note tone="caution" title="The scan did not finish.">
              {storage.scanError}
            </Note>
          )}
          {storage.selectedVolume && !storage.selectedVolume.isReadable && (
            <Note tone="caution" title="Mr Clean cannot read this volume.">
              macOS has not granted access to it. On the boot volume this usually means Full
              Disk Access is off; you can turn it on in System Settings under Privacy &amp;
              Security, or choose a folder instead.
            </Note>
          )}
        </div>
      )}
    </>
  );
}
